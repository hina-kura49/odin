//! メモアプリのバックエンドの中核。Tauri に依存しない。

mod disk;
mod index;
mod links;
mod names;
mod search;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use disk::{Kind, find_entry, locate, locate_page};
use index::Index;
use names::{key, nfc, process_title, stem};

/// write_page が使う一時ファイルの名前の接頭辞。`.` で始まり、一時ファイルの名前は `.md` で終わらない。
/// open のときに、この接頭辞で始まる残った一時ファイルを vault 全体から削除する。
/// 値はアプリ名が決まったら変えてよい(テストはこの定数を参照する)。
pub const TEMP_FILE_PREFIX: &str = ".odin-tmp-";

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// 指定したパスに何も存在しない。
    #[error("not found: {0}")]
    NotFound(String),
    /// 正規形でないパス(`..`、`.`、空の要素、先頭の `/`、絶対パスなど)。
    #[error("invalid path: {0}")]
    InvalidPath(String),
    /// 存在はするが操作の対象にならないもの
    /// (隠しファイル、.md 以外、シンボリックリンク、Folder の節点)。
    #[error("not a page: {0}")]
    NotAPage(String),
    /// ページだが UTF-8 として読めない。
    #[error("not valid UTF-8: {0}")]
    NotUtf8(String),
    /// 読み取り専用のファイルには書き込まない。
    #[error("read only: {0}")]
    ReadOnly(String),
    /// 作ろうとした場所に、ページでもフォルダでもない同名のもの(拡張子のないファイルなど)がある。
    /// それを動かしたり上書きしたりしない。
    #[error("name occupied: {0}")]
    NameOccupied(String),
    /// index_dir が vault の中にある、または vault が index_dir の中にある
    /// (シンボリックリンクを解決した後で判定)。
    #[error("index dir overlaps vault: {0}")]
    IndexOverlapsVault(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMeta {
    pub path: String,
    pub title: String,
    /// UNIX 時刻のミリ秒。表示と recent_pages の並びに使う。
    pub modified_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Page,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub kind: NodeKind,
    pub path: String,
    pub title: String,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub snippet: Snippet,
    /// 回答61の「タイトル一致」(検索語のすべてがタイトルに含まれる)のとき true、それ以外は false。
    pub title_matched: bool,
}

/// 検索結果に添える本文の抜粋。強調の記号は入れない。
/// - `hit`: 本文で最初に現れる一致箇所(本文の元の表記のまま)。
/// - `before`: 一致の直前の最大30文字。全体(before + hit + after)は最大120文字。文字は書記素で数える。
/// - 改行と連続する空白は半角の空白1つにまとめ、切り詰めた側にだけ「…」を付ける(文字数に含めない)。
/// - タイトルだけに一致したときは、`before` と `hit` が空で、`after` に本文の先頭を入れる。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snippet {
    pub before: String,
    pub hit: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteResult {
    Ok { version: String },
    Conflict,
}

/// ページをゴミ箱へ移す処理。テストでは OS のゴミ箱を汚さないように差し替える。
pub trait Trash: Send + Sync {
    /// `path`(ファイルまたはフォルダ)を、中身ごとゴミ箱へ移す。
    fn trash(&self, path: &Path) -> std::io::Result<()>;
}

/// macOS のゴミ箱。`Vault::open` はこれを使う。
pub struct SystemTrash;

impl Trash for SystemTrash {
    fn trash(&self, path: &Path) -> std::io::Result<()> {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        // Finder を使う方法は、アプリに「Finder の操作の許可」を求めてしまうので使わない。
        let mut context = trash::TrashContext::default();
        context.set_delete_method(DeleteMethod::NsFileManager);
        context.delete(path).map_err(std::io::Error::other)
    }
}

pub struct Vault {
    // vault のフォルダ
    root: PathBuf,
    // ページを消すときに使うゴミ箱。
    trash: Box<dyn Trash>,
    // ページの一覧と本文。ページを変える操作は、この鍵を持っている間に行う。
    index: Mutex<Index>,
}

impl Drop for Vault {
    fn drop(&mut self) {
        self.lock().save();
    }
}

impl Vault {
    /// `index_dir` は vault の外に置く索引用のフォルダ。
    /// 存在しなければ途中も含めて作る。
    pub fn open(root: &Path, index_dir: &Path) -> Result<Vault> {
        Vault::open_with_trash(root, index_dir, Box::new(SystemTrash))
    }

    /// ゴミ箱の処理を差し替えて開く。それ以外は`open`と同じ。
    pub fn open_with_trash(root: &Path, index_dir: &Path, trash: Box<dyn Trash>) -> Result<Vault> {
        // rootがフォルダでなければエラーにする。
        if !root.is_dir() {
            return Err(Error::NotFound(root.display().to_string()));
        }

        // vault と index_dir を、シンボリックリンクを解決した絶対パスにしてから比べる。
        let resolved_root = resolve_path(root)?;
        let resolved_index = resolve_path(index_dir)?;

        // どちらかがもう一方の中にある（同じ場合も含む）なら、重なっているのでエラーにする。
        // 何かを作る前に調べるので、エラーのときは何も作られない。
        if resolved_index.starts_with(&resolved_root) || resolved_root.starts_with(&resolved_index)
        {
            return Err(Error::IndexOverlapsVault(index_dir.display().to_string()));
        }

        // index_dir がなければ、途中のフォルダも含めて作る。
        std::fs::create_dir_all(index_dir)?;

        // 保存してある索引を読み、ディスクと突き合わせる。残った一時ファイルはここで消す。
        let mut index = Index::load(index_dir);
        index.scan(root, false, true)?;
        index.save();

        Ok(Vault {
            root: root.to_path_buf(),
            trash,
            index: Mutex::new(index),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Index> {
        // 途中で panic した操作があっても、索引はディスクと突き合わせれば直せるので使い続ける。
        self.index.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 索引の中のページの情報。無ければディスクから作る。
    fn page_meta(&self, index: &mut Index, disk_rel: &str) -> Result<PageMeta> {
        if let Some(meta) = index.meta(disk_rel) {
            return Ok(meta);
        }
        index.refresh(&self.root, disk_rel)?;
        index
            .meta(disk_rel)
            .ok_or_else(|| Error::NotFound(nfc(disk_rel)))
    }

    pub fn list_tree(&self) -> Result<Vec<TreeNode>> {
        Ok(self.lock().list_tree())
    }
    /// 内容と version を返す。version は内容のハッシュに基づく不透明な文字列。
    pub fn read_page(&self, path: &str) -> Result<(String, String)> {
        // vaultのフォルダとpathをつなげて、ファイルの場所を作る。
        // パスの書き方が正しいかを先に調べる。だめならここで終わる。
        validate_path(path)?;

        // ページとして読んで良いものかを調べる。
        check_is_page(&self.root, path)?;

        let full_path = self.root.join(path);

        // ファイルを読んで、バイト列(Vec<u8>)として受け取る。
        let bytes = match std::fs::read(&full_path) {
            // 読めたら、その中身を使う。
            Ok(bytes) => bytes,
            // ファイルがなかったらNotFoundにする。
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(Error::NotFound(path.to_string()));
            }
            // それ以外の失敗はIoのまま返す。
            Err(e) => return Err(Error::Io(e)),
        };

        // 中身のバイト列から version を作る
        // from_utf8 が bytes を使い切る前に作っておく。
        let version = version_of(&bytes);

        // バイト列を文字列にする。UTF-8として読めなければNotUtf8にする。
        let content = match String::from_utf8(bytes) {
            Ok(content) => content,
            Err(_) => return Err(Error::NotUtf8(path.to_string())),
        };

        Ok((content, version))
    }

    /// ディスク上の現在の内容の version が `base_version` と異なれば `Conflict` を返し、
    /// ファイルを変更しない。書き込みは一時ファイルに書いてから rename する。
    pub fn write_page(&self, path: &str, content: &str, base_version: &str) -> Result<WriteResult> {
        let mut index = self.lock();
        let page = locate_page(&self.root, path)?;
        let abs = self.root.join(page.disk_rel());

        let meta = read_meta(&abs, path)?;
        let current = read_bytes(&abs, path)?;
        // UTF-8 でないページには、base_version によらず書かない(回答19)。
        if std::str::from_utf8(&current).is_err() {
            return Err(Error::NotUtf8(path.to_string()));
        }
        if version_of(&current) != base_version {
            return Ok(WriteResult::Conflict);
        }
        if meta.permissions().readonly() {
            return Err(Error::ReadOnly(path.to_string()));
        }

        if current != content.as_bytes() {
            disk::replace_file(&abs, content.as_bytes(), Some(&meta))?;
            index.refresh(&self.root, &page.disk_rel())?;
        }
        Ok(WriteResult::Ok {
            version: version_of(content.as_bytes()),
        })
    }

    /// `parent` はページの path か Folder の節点の path。
    /// ページなら子はそれと同名のフォルダの中に、Folder ならそのフォルダの中に作る。
    /// 新しいページの中身は空(0バイト)。
    pub fn create_page(&self, parent: Option<&str>, title: &str) -> Result<PageMeta> {
        let mut index = self.lock();

        // 新しいページを置くフォルダ(ディスク上の名前)と、作る必要があるか。
        let (dir, create_dir): (Vec<String>, bool) = match parent {
            None => (Vec::new(), false),
            Some(parent) => {
                let located = locate(&self.root, parent)?;
                match located.kind {
                    Kind::Page => {
                        // ページの子は、ページと同じ名前のフォルダに置く。
                        let parent_dir = self.root.join(located.parent().join("/"));
                        let folder = stem(located.name()).to_string();
                        let mut dir = located.parent().to_vec();
                        match find_entry(&parent_dir, &folder)? {
                            Some((name, file_type)) if file_type.is_dir() => {
                                dir.push(name);
                                (dir, false)
                            }
                            // フォルダを作る場所に、フォルダでないものがある(回答32)。
                            Some((name, _)) => return Err(Error::NameOccupied(nfc(&name))),
                            None => {
                                dir.push(folder);
                                (dir, true)
                            }
                        }
                    }
                    // ページを含むフォルダだけが Folder の節点(回答15・24)。
                    Kind::Dir if disk::has_pages(&self.root, &located.disk_rel())? => {
                        (located.segments, false)
                    }
                    Kind::Dir | Kind::Other => return Err(Error::NotAPage(parent.to_string())),
                }
            }
        };

        let dir_rel = dir.join("/");
        let dir_abs = self.root.join(&dir_rel);
        if create_dir {
            fs::create_dir(&dir_abs)?;
        }

        let result = (|| {
            let title = self.free_title(&dir_abs, &process_title(title), create_dir)?;
            let name = format!("{title}.md");
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dir_abs.join(&name))?;
            Ok(if dir_rel.is_empty() {
                name
            } else {
                format!("{dir_rel}/{name}")
            })
        })();
        let disk_rel = match result {
            Ok(disk_rel) => disk_rel,
            Err(e) => {
                if create_dir {
                    let _ = fs::remove_dir(&dir_abs);
                }
                return Err(e);
            }
        };
        index.refresh(&self.root, &disk_rel)?;
        self.page_meta(&mut index, &disk_rel)
    }

    /// 同じフォルダのページと重ならないタイトル。重なれば「 2」から順に、空いている最小の番号を付ける(回答28)。
    fn free_title(&self, dir: &Path, title: &str, dir_is_new: bool) -> Result<String> {
        let taken: std::collections::HashSet<String> = if dir_is_new {
            Default::default()
        } else {
            fs::read_dir(dir)?
                .filter_map(|e| e.ok()?.file_name().into_string().ok())
                .map(|name| key(&name))
                .collect()
        };
        let mut candidate = title.to_string();
        let mut n = 2;
        while taken.contains(&key(&format!("{candidate}.md"))) {
            candidate = format!("{title} {n}");
            n += 1;
        }
        Ok(candidate)
    }

    /// 子フォルダ(中身を問わない)を先に、ページを後にゴミ箱へ移す。元に戻す処理はしない。
    /// その結果、親のフォルダが空になったら、ルートを除いて上へ順に削除する。
    pub fn delete_page(&self, path: &str) -> Result<()> {
        let mut index = self.lock();
        let page = locate_page(&self.root, path)?;
        let parent_abs = self.root.join(page.parent().join("/"));

        // 子フォルダ(回答23)。シンボリックリンクはたどらない。
        if let Some((folder, file_type)) = find_entry(&parent_abs, stem(page.name()))?
            && file_type.is_dir()
        {
            self.trash.trash(&parent_abs.join(&folder))?;
            let mut folder_rel = page.parent().to_vec();
            folder_rel.push(folder);
            index.remove_dir(&folder_rel.join("/"));
        }

        self.trash.trash(&self.root.join(page.disk_rel()))?;
        index.remove(&page.disk_rel());

        // 空になった親フォルダを、ルートの手前まで上へ順に削除する(回答25)。
        let mut dir = page.parent().to_vec();
        while !dir.is_empty() {
            let abs = self.root.join(dir.join("/"));
            if fs::read_dir(&abs)?.next().is_some() {
                break;
            }
            fs::remove_dir(&abs)?;
            dir.pop();
        }
        Ok(())
    }

    /// ファイル名を変え、子フォルダがあれば一緒に変え、vault 内の他のページからのリンクを書き換える。
    /// リンク以外の本文は1バイトも変えない。
    pub fn rename_page(&self, path: &str, new_title: &str) -> Result<PageMeta> {
        let mut index = self.lock();
        let page = locate_page(&self.root, path)?;
        let old_name = page.name().to_string();
        let old_stem = stem(&old_name).to_string();
        let new_title = process_title(new_title);

        // まったく同じ名前なら、何も変えない(回答43)。
        if nfc(&old_stem) == new_title {
            return self.page_meta(&mut index, &page.disk_rel());
        }

        let parent = page.parent().to_vec();
        let parent_abs = self.root.join(parent.join("/"));
        let new_name = format!("{new_title}.md");

        // 新しい名前のページがあれば、番号を付けずにエラーにする(回答35)。大文字小文字だけの変更は除く。
        if key(&new_name) != key(&old_name) && find_entry(&parent_abs, &new_name)?.is_some() {
            return Err(Error::NameOccupied(new_title));
        }
        let folder = match find_entry(&parent_abs, &old_stem)? {
            Some((name, file_type)) if file_type.is_dir() => Some(name),
            _ => None,
        };
        // 子フォルダがあるときは、新しい名前のフォルダの場所が空いていなければならない。
        if folder.is_some()
            && key(&new_title) != key(&old_stem)
            && find_entry(&parent_abs, &new_title)?.is_some()
        {
            return Err(Error::NameOccupied(new_title));
        }

        let target = links::Target {
            parent: parent.iter().map(|s| key(s)).collect(),
            page: key(&old_name),
            folder: folder.as_deref().map(key),
            old_title: nfc(&old_stem),
            new_title: new_title.clone(),
        };

        // 書き換えが要るページを集める。読み取り専用のものがあれば、何も変えずにエラーにする(回答39)。
        let mut pages = Vec::new();
        disk::walk_pages(&self.root, "", false, &mut pages)?;
        let mut rewrites = Vec::new();
        for (rel, meta) in pages {
            let Ok(content) = String::from_utf8(fs::read(self.root.join(&rel))?) else {
                continue;
            };
            let base: Vec<String> = rel.split('/').map(key).collect();
            let Some(new_content) = links::rewrite(&content, &base[..base.len() - 1], &target)
            else {
                continue;
            };
            if meta.permissions().readonly() {
                return Err(Error::ReadOnly(nfc(&rel)));
            }
            rewrites.push((rel, meta, new_content));
        }

        // 1. 書き換えた中身を、それぞれのページと同じフォルダの一時ファイルに書く。
        let mut temps = Vec::new();
        for (rel, meta, new_content) in &rewrites {
            match disk::write_temp(&self.root.join(rel), new_content.as_bytes(), Some(meta)) {
                Ok(temp) => temps.push((rel.clone(), temp)),
                Err(e) => {
                    remove_all(temps.iter().map(|(_, t)| t.clone()));
                    return Err(e.into());
                }
            }
        }

        // 2. 子フォルダとページの名前を変える。失敗したら元に戻す(回答41)。
        let old_folder_abs = folder.as_ref().map(|f| parent_abs.join(f));
        let new_folder_abs = parent_abs.join(&new_title);
        if let Some(old_folder_abs) = &old_folder_abs
            && let Err(e) = fs::rename(old_folder_abs, &new_folder_abs)
        {
            remove_all(temps.iter().map(|(_, t)| t.clone()));
            return Err(e.into());
        }
        if let Err(e) = fs::rename(parent_abs.join(&old_name), parent_abs.join(&new_name)) {
            if let Some(old_folder_abs) = &old_folder_abs {
                let _ = fs::rename(&new_folder_abs, old_folder_abs);
            }
            remove_all(
                temps
                    .iter()
                    .map(|(_, t)| moved(t, old_folder_abs.as_deref(), &new_folder_abs)),
            );
            return Err(e.into());
        }

        // 3. 一時ファイルで、書き換えたページを置き換える。
        let old_page_rel = page.disk_rel();
        let old_folder_rel = folder.as_ref().map(|f| join_rel(&parent, f));
        let new_page_rel = join_rel(&parent, &new_name);
        let new_folder_rel = join_rel(&parent, &new_title);
        let mut first_error = None;
        for (rel, temp) in &temps {
            let final_rel = if *rel == old_page_rel {
                new_page_rel.clone()
            } else {
                match &old_folder_rel {
                    Some(old) if rel.starts_with(&format!("{old}/")) => {
                        format!("{new_folder_rel}{}", &rel[old.len()..])
                    }
                    _ => rel.clone(),
                }
            };
            let temp = moved(temp, old_folder_abs.as_deref(), &new_folder_abs);
            if let Err(e) = fs::rename(&temp, self.root.join(&final_rel)) {
                let _ = fs::remove_file(&temp);
                first_error.get_or_insert(e);
            }
            index.refresh(&self.root, &final_rel)?;
        }

        // 索引を新しい名前に合わせる。
        index.remove(&old_page_rel);
        if let Some(old) = &old_folder_rel {
            index.remove_dir(old);
            index.refresh_dir(&self.root, &new_folder_rel)?;
        }
        index.refresh(&self.root, &new_page_rel)?;
        if let Some(e) = first_error {
            return Err(e.into());
        }
        self.page_meta(&mut index, &new_page_rel)
    }

    /// タイトルと本文(ファイルの全文)から探し、並び順の上位 `limit` 件を返す。
    /// 比較は NFKC にそろえ、英字の大文字小文字を区別しない。空白で区切った語はすべてを含むページを探す。
    /// 並び順: タイトル一致 > 本文だけの一致、同じ順位では modified_at の新しい順、次に path 順。
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        Ok(self.lock().search(query, limit))
    }

    /// 更新日時(modified_at)の新しい順(同じなら path 順)に、最大 `limit` 件を返す。
    pub fn recent_pages(&self, limit: usize) -> Result<Vec<PageMeta>> {
        Ok(self.lock().recent_pages(limit))
    }

    /// vault のファイルから索引を作り直す。
    pub fn rebuild_index(&self) -> Result<()> {
        let mut index = self.lock();
        index.scan(&self.root, true, false)?;
        index.save();
        Ok(())
    }

    /// アプリの外での追加・変更・削除を、list_tree・search・recent_pages に反映する。
    /// 大きさと mtime が同じで中身だけ違う変更は見逃してよい(rebuild_index では反映される)。
    pub fn rescan(&self) -> Result<()> {
        let mut index = self.lock();
        index.scan(&self.root, false, false)?;
        index.save();
        Ok(())
    }

    /// vault 直下の `Inbox.md` の末尾に、空行を挟んで追記する。なければ作る。
    pub fn capture_to_inbox(&self, text: &str) -> Result<()> {
        // (a) 前後の空白を取り除く。(b) 空なら何もしない。
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        let mut index = self.lock();

        let (name, existing, meta) = match find_entry(&self.root, INBOX)? {
            None => (INBOX.to_string(), Vec::new(), None),
            Some((name, file_type)) => {
                if !file_type.is_file() {
                    return Err(Error::NotAPage(nfc(&name)));
                }
                let abs = self.root.join(&name);
                let meta = read_meta(&abs, &name)?;
                let existing = read_bytes(&abs, &name)?;
                if std::str::from_utf8(&existing).is_err() {
                    return Err(Error::NotUtf8(nfc(&name)));
                }
                if meta.permissions().readonly() {
                    return Err(Error::ReadOnly(nfc(&name)));
                }
                (name, existing, Some(meta))
            }
        };

        let existing = String::from_utf8(existing).expect("UTF-8 であることを確かめた");
        let content = append_to_inbox(&existing, text);
        disk::replace_file(&self.root.join(&name), content.as_bytes(), meta.as_ref())?;
        index.refresh(&self.root, &name)?;
        Ok(())
    }
}

/// Inbox のファイル名。
const INBOX: &str = "Inbox.md";

/// 既存の Inbox の中身に text を足した中身(手順 (c)〜(e))。
fn append_to_inbox(existing: &str, text: &str) -> String {
    // (c) ない、または0バイトなら「text + 改行」。
    if existing.is_empty() {
        return format!("{text}\n");
    }
    // (e) 最初に現れる改行が CRLF なら CRLF、それ以外は LF。
    let newline = match existing.find('\n') {
        Some(i) if existing[..i].ends_with('\r') => "\r\n",
        _ => "\n",
    };
    // (d) 末尾の改行を数える(CRLF は1つ)。2個以上なら何も足さない。
    let mut rest = existing;
    let mut trailing = 0;
    while trailing < 2 {
        if let Some(r) = rest
            .strip_suffix("\r\n")
            .or_else(|| rest.strip_suffix('\n'))
        {
            rest = r;
            trailing += 1;
        } else {
            break;
        }
    }
    let separator = newline.repeat(2 - trailing);
    format!("{existing}{separator}{text}{newline}")
}

/// ルートからの名前の並びに、名前を1つ足した相対パス。
fn join_rel(dir: &[String], name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{}/{name}", dir.join("/"))
    }
}

/// 子フォルダの名前を変えた後の、一時ファイルの場所。
fn moved(temp: &Path, old_folder: Option<&Path>, new_folder: &Path) -> PathBuf {
    match old_folder.and_then(|old| temp.strip_prefix(old).ok()) {
        Some(rest) => new_folder.join(rest),
        None => temp.to_path_buf(),
    }
}

/// 一時ファイルを消す(失敗は無視する)。
fn remove_all(paths: impl Iterator<Item = PathBuf>) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

/// ファイルの情報を読む。無ければ NotFound。
fn read_meta(abs: &Path, path: &str) -> Result<fs::Metadata> {
    fs::symlink_metadata(abs).map_err(|e| not_found_or_io(e, path))
}

/// ファイルの中身を読む。無ければ NotFound。
fn read_bytes(abs: &Path, path: &str) -> Result<Vec<u8>> {
    fs::read(abs).map_err(|e| not_found_or_io(e, path))
}

fn not_found_or_io(e: std::io::Error, path: &str) -> Error {
    if e.kind() == std::io::ErrorKind::NotFound {
        Error::NotFound(path.to_string())
    } else {
        Error::Io(e)
    }
}

/// アプリから受け取ったパスが、決まった書き方になっているかを調べる。
/// 正しければOk(())、だめならErr(Error::InvalidPath)を返す。
fn validate_path(path: &str) -> Result<()> {
    // だめだった時に返すエラーを、先に用意しておく。
    let invalid = || Error::InvalidPath(path.to_string());

    // 空のパスはだめ
    if path.is_empty() {
        return Err(invalid());
    }

    // 先頭が"/"はだめ。macOSの絶対パスは全て"/"で始まるので、絶対パスもここで弾ける。
    if path.starts_with('/') {
        return Err(invalid());
    }

    // 1文字ずつ見て、"\"と制御文字があればだめ。
    for c in path.chars() {
        if c == '\\' || c.is_control() {
            return Err(invalid());
        }
    }

    // "/"で区切った１つずつの部分（セグメント）を調べる。
    for segment in path.split('/') {
        // 空（「a//b.md」や末尾の「/」）、"."、".."はだめ。
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(invalid());
        }
    }

    Ok(())
}

// パスが指すものがページかを調べる。
// 隠しファイル、隠しフォルダの中、.md以外、フォルダ、シンボリックリンクはページではない。
// みつからないときは何もしない（後で読む時にNotFoundになる）
fn check_is_page(root: &Path, path: &str) -> Result<()> {
    // ページではなかった時に返すエラーを先に用意しておく。
    let not_a_page = || Error::NotAPage(path.to_string());

    // 名前だけで決まることを先に調べる。
    // "."
    // "."で始まる部分があれば、隠しファイルか隠しフォルダの中なのでだめ。
    for segment in path.split('/') {
        if segment.starts_with(".") {
            return Err(not_a_page());
        }
    }

    // 拡張子が.mdでなければだめ。大文字小文字は問わない。（.MDでも良い）
    if !path.to_lowercase().ends_with(".md") {
        return Err(not_a_page());
    }

    // ディスクを vaultのフォルダから１段ずつ辿って、シンボリックリンクがないかを調べる。
    let mut current = root.to_path_buf();
    let mut is_file = false;
    for segment in path.split('/') {
        current.push(segment);

        // symlink_metadata は、リンクの先ではなくリンクそのものの情報を返す。
        let meta = match std::fs::symlink_metadata(&current) {
            Ok(meta) => meta,
            // 見つからなければ、ここでは判断しない。
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(Error::Io(e)),
        };

        // 途中のフォルダでも、最後のファイルでも、シンボリックならだめ。
        if meta.file_type().is_symlink() {
            return Err(not_a_page());
        }

        is_file = meta.is_file();
    }

    // 最後にたどり着いたものがファイルでなければ（"x.md"という名前のフォルダなど）だめ。
    if !is_file {
        return Err(not_a_page());
    }

    Ok(())
}

/// ファイルの中身（バイト列）から、 version を作る。
/// 中身が１バイトでも違えば、別の version になる。
fn version_of(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    // 中身から SHA-256 のハッシュ(32バイト)を計算する。
    let hash = Sha256::digest(bytes);

    // 32バイトを、16進数の文字列(64文字)にする。
    let mut text = String::new();
    for byte in hash {
        text.push_str(&format!("{:02x}", byte));
    }
    text
}

/// パスを、シンボリックリンクを解決した絶対パスにする。
/// まだ存在しない部分があっても使えるように、存在する親までを解決して、残りを繋げ直す。
fn resolve_path(path: &Path) -> Result<PathBuf> {
    // 存在しない末尾の名前を、ここに集める（後ろから順に入る）。
    let mut missing = Vec::new();
    let mut current = path.to_path_buf();

    loop {
        match std::fs::canonicalize(&current) {
            // 解決できたら、集めておいた名前を元の順に繋げ直して終わる。
            Ok(resolved) => {
                let mut result = resolved;
                for name in missing.iter().rev() {
                    result.push(name);
                }
                return Ok(result);
            }
            // 存在しなければ、末尾の名前を１つ外して、親で試し直す。
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let name = match current.file_name() {
                    Some(name) => name.to_os_string(),
                    // もう外せる名前がない（".."や空のパスなど）時は諦める。
                    None => return Err(Error::Io(e)),
                };
                missing.push(name);
                current.pop();
            }
            Err(e) => return Err(Error::Io(e)),
        }
    }
}
