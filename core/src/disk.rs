//! ディスクの操作: パスをディスク上の名前にたどる、ページを集める、書き込む。

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::names::{is_md_name, is_visible_name, key};
use crate::{Error, Result, TEMP_FILE_PREFIX, validate_path};

/// パスが指していたものの種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// .md のファイル
    Page,
    /// フォルダ
    Dir,
    /// それ以外のファイル
    Other,
}

/// ディスク上の名前にたどったパス。
pub(crate) struct Located {
    /// ルートからの、ディスク上の表記のままの名前
    pub segments: Vec<String>,
    pub kind: Kind,
}

impl Located {
    /// ディスク上の表記のままの相対パス(「/」区切り)。
    pub fn disk_rel(&self) -> String {
        self.segments.join("/")
    }

    /// 最後の名前(ページならファイル名)。
    pub fn name(&self) -> &str {
        self.segments.last().map(String::as_str).unwrap_or("")
    }

    /// 親フォルダの、ルートからの名前。
    pub fn parent(&self) -> &[String] {
        &self.segments[..self.segments.len() - 1]
    }
}

/// `dir` の中から、名前が `name` と同じもの(NFC と大文字小文字の違いは無視する)を探す。
/// 見つかれば、ディスク上の名前と種類を返す。種類はシンボリックリンクをたどらない。
pub(crate) fn find_entry(dir: &Path, name: &str) -> io::Result<Option<(String, fs::FileType)>> {
    let wanted = key(name);
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let Ok(found) = entry.file_name().into_string() else {
            continue;
        };
        if key(&found) == wanted {
            return Ok(Some((found, entry.file_type()?)));
        }
    }
    Ok(None)
}

/// アプリから受け取った path を、ディスク上の名前にたどる。
/// - 書き方が正規形でなければ InvalidPath
/// - 隠しファイルか隠しフォルダの中、途中にシンボリックリンクがあれば NotAPage
/// - 見つからなければ NotFound
pub(crate) fn locate(root: &Path, path: &str) -> Result<Located> {
    validate_path(path)?;

    // "." で始まる部分があれば、隠しファイルか隠しフォルダの中なので対象外。
    if path.split('/').any(|segment| segment.starts_with('.')) {
        return Err(Error::NotAPage(path.to_string()));
    }

    let names: Vec<&str> = path.split('/').collect();
    let mut segments = Vec::new();
    let mut current = root.to_path_buf();
    for (i, name) in names.iter().enumerate() {
        let last = i == names.len() - 1;
        let Some((found, file_type)) = find_entry(&current, name)? else {
            return Err(Error::NotFound(path.to_string()));
        };
        if file_type.is_symlink() {
            return Err(Error::NotAPage(path.to_string()));
        }
        current.push(&found);
        segments.push(found);
        if last {
            let kind = if file_type.is_dir() {
                Kind::Dir
            } else if is_md_name(&segments[i]) {
                Kind::Page
            } else {
                Kind::Other
            };
            return Ok(Located { segments, kind });
        }
        if !file_type.is_dir() {
            return Err(Error::NotFound(path.to_string()));
        }
    }
    unreachable!("validate_path は空のパスを通さない")
}

/// ページだけを受け付けるときの locate。フォルダや .md 以外のファイルは NotAPage。
pub(crate) fn locate_page(root: &Path, path: &str) -> Result<Located> {
    let located = locate(root, path)?;
    if located.kind != Kind::Page {
        return Err(Error::NotAPage(path.to_string()));
    }
    Ok(located)
}

/// `dir`(ルートからの相対パス)以下のページを、ディスク上の相対パスと情報の組で集める。
/// 隠しファイル・隠しフォルダ・使えない文字を含む名前・シンボリックリンクは対象外。
/// `remove_temp_files` が true なら、残った一時ファイルを見つけしだい削除する。
pub(crate) fn walk_pages(
    root: &Path,
    dir: &str,
    remove_temp_files: bool,
    out: &mut Vec<(String, fs::Metadata)>,
) -> io::Result<()> {
    let abs = if dir.is_empty() {
        root.to_path_buf()
    } else {
        root.join(dir)
    };
    let entries = match fs::read_dir(&abs) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let file_type = entry.file_type()?;
        if !is_visible_name(&name) {
            if remove_temp_files && name.starts_with(TEMP_FILE_PREFIX) && file_type.is_file() {
                let _ = fs::remove_file(entry.path());
            }
            continue;
        }
        let rel = if dir.is_empty() {
            name.clone()
        } else {
            format!("{dir}/{name}")
        };
        if file_type.is_dir() {
            walk_pages(root, &rel, remove_temp_files, out)?;
        } else if file_type.is_file() && is_md_name(&name) {
            out.push((rel, entry.metadata()?));
        }
    }
    Ok(())
}

/// フォルダ(ルートからの相対パス)の中に、ページが1つでもあるか。
pub(crate) fn has_pages(root: &Path, dir: &str) -> io::Result<bool> {
    let mut pages = Vec::new();
    walk_pages(root, dir, false, &mut pages)?;
    Ok(!pages.is_empty())
}

/// 更新日時を UNIX 時刻のミリ秒で返す。
pub(crate) fn modified_ms(meta: &fs::Metadata) -> u64 {
    modified_ns(meta) as u64 / 1_000_000
}

/// 更新日時を UNIX 時刻のナノ秒で返す(変更を見つけるために使う)。
pub(crate) fn modified_ns(meta: &fs::Metadata) -> u128 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// 同じフォルダに置く一時ファイルの名前。
fn temp_path(dir: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.join(format!(
        "{TEMP_FILE_PREFIX}{}-{nanos}-{n}",
        std::process::id()
    ))
}

/// `target` と同じフォルダの一時ファイルに `bytes` を書き、一時ファイルのパスを返す。
/// `original` があれば、その権限・拡張属性・作成日を一時ファイルに写す(回答20)。
/// 失敗したら一時ファイルを残さない。
pub(crate) fn write_temp(
    target: &Path,
    bytes: &[u8],
    original: Option<&fs::Metadata>,
) -> io::Result<PathBuf> {
    let dir = target
        .parent()
        .ok_or_else(|| io::Error::other("親フォルダがない"))?;
    let temp = temp_path(dir);
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        if let Some(original) = original {
            file.set_permissions(original.permissions())?;
            copy_xattrs(target, &temp);
            // macOS では、mtime を作成日より前にすると作成日もその時刻になる。
            // いったん元の作成日にしてから、今の時刻に戻す。
            if let Ok(created) = original.created() {
                file.set_modified(created)?;
                file.set_modified(SystemTime::now())?;
            }
        }
        file.sync_all()
    })();
    match result {
        Ok(()) => Ok(temp),
        Err(e) => {
            let _ = fs::remove_file(&temp);
            Err(e)
        }
    }
}

/// 拡張属性(Finder のタグなど)を写す。写せないものは飛ばす。
fn copy_xattrs(from: &Path, to: &Path) {
    let Ok(names) = xattr::list(from) else {
        return;
    };
    for name in names {
        if let Ok(Some(value)) = xattr::get(from, &name) {
            let _ = xattr::set(to, &name, &value);
        }
    }
}

/// 一時ファイルに書いてから rename して、`target` の中身を置き換える(原子的な書き込み)。
pub(crate) fn replace_file(
    target: &Path,
    bytes: &[u8],
    original: Option<&fs::Metadata>,
) -> io::Result<()> {
    let temp = write_temp(target, bytes, original)?;
    fs::rename(&temp, target).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}
