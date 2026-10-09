//! 索引: vault のページの一覧と本文をメモリに持ち、index_dir にキャッシュとして保存する。
//!
//! 保存したものは、次に開くときに読み込みを速くするためだけに使う。
//! 開くたびにディスクと突き合わせ(大きさと mtime が違うページだけ読み直す)、
//! 保存したものが壊れていたり無かったりしても、ディスクから作り直すので結果は変わらない。

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::disk::{self, modified_ms, modified_ns};
use crate::names::{key, natural_cmp, natural_path_cmp, nfc, stem};
use crate::search::{fold, query_words, snippet};
use crate::{NodeKind, PageMeta, SearchHit, TreeNode};

const INDEX_FILE: &str = "index.bin";
const MAGIC: &[u8; 8] = b"ODINIDX1";

/// 索引の中の1ページ。
pub(crate) struct Entry {
    /// ディスク上の表記のままの相対パス
    disk: String,
    /// アプリに返す path(NFC)
    path: String,
    title: String,
    folded_title: String,
    size: u64,
    mtime_ns: u128,
    modified_at: u64,
    /// UTF-8 として読めたときだけ、本文とその比べるための形を持つ
    text: Option<Text>,
}

struct Text {
    body: String,
    folded: String,
}

impl Entry {
    fn new(
        disk: String,
        size: u64,
        mtime_ns: u128,
        modified_at: u64,
        body: Option<String>,
    ) -> Entry {
        let path = nfc(&disk);
        let name = path.rsplit('/').next().unwrap_or(&path);
        let title = stem(name).to_string();
        let text = body.map(|body| Text {
            folded: fold(&body),
            body,
        });
        Entry {
            folded_title: fold(&title),
            disk,
            path,
            title,
            size,
            mtime_ns,
            modified_at,
            text,
        }
    }

    fn read(root: &Path, disk: &str, meta: &fs::Metadata) -> io::Result<Entry> {
        let bytes = fs::read(root.join(disk))?;
        Ok(Entry::new(
            disk.to_string(),
            meta.len(),
            modified_ns(meta),
            modified_ms(meta),
            String::from_utf8(bytes).ok(),
        ))
    }

    fn meta(&self) -> PageMeta {
        PageMeta {
            path: self.path.clone(),
            title: self.title.clone(),
            modified_at: self.modified_at,
        }
    }
}

pub(crate) struct Index {
    file: PathBuf,
    entries: BTreeMap<String, Entry>,
    /// 保存したものと中身が違うか
    dirty: bool,
}

impl Index {
    /// 保存したものを読み込む。無い・壊れているときは空から始める。
    pub fn load(index_dir: &Path) -> Index {
        let file = index_dir.join(INDEX_FILE);
        match fs::read(&file).ok().and_then(|bytes| decode(&bytes)) {
            Some(entries) => Index {
                file,
                entries,
                dirty: false,
            },
            None => Index {
                file,
                entries: BTreeMap::new(),
                dirty: true,
            },
        }
    }

    /// ディスクと突き合わせる。`full` なら、すべてのページを読み直す。
    pub fn scan(&mut self, root: &Path, full: bool, remove_temp_files: bool) -> io::Result<()> {
        let mut found = Vec::new();
        disk::walk_pages(root, "", remove_temp_files, &mut found)?;
        let mut entries = BTreeMap::new();
        for (rel, meta) in found {
            let reuse = !full
                && self
                    .entries
                    .get(&rel)
                    .is_some_and(|e| e.size == meta.len() && e.mtime_ns == modified_ns(&meta));
            let entry = match self.entries.remove(&rel) {
                Some(e) if reuse => e,
                _ => {
                    self.dirty = true;
                    match Entry::read(root, &rel, &meta) {
                        Ok(e) => e,
                        // 走査の間に消えたページは飛ばす。
                        Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                        Err(e) => return Err(e),
                    }
                }
            };
            entries.insert(rel, entry);
        }
        if !self.entries.is_empty() {
            self.dirty = true;
        }
        self.entries = entries;
        Ok(())
    }

    /// 1ページを読み直す。無くなっていれば索引から外す。
    pub fn refresh(&mut self, root: &Path, disk: &str) -> io::Result<()> {
        self.dirty = true;
        match fs::symlink_metadata(root.join(disk)) {
            Ok(meta) if meta.is_file() => {
                let entry = Entry::read(root, disk, &meta)?;
                self.entries.insert(disk.to_string(), entry);
                Ok(())
            }
            Ok(_) => {
                self.entries.remove(disk);
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                self.entries.remove(disk);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// フォルダ(ディスク上の相対パス)以下のページを、ディスクから読み直す。
    pub fn refresh_dir(&mut self, root: &Path, dir: &str) -> io::Result<()> {
        self.remove_dir(dir);
        let mut found = Vec::new();
        disk::walk_pages(root, dir, false, &mut found)?;
        for (rel, meta) in found {
            let entry = Entry::read(root, &rel, &meta)?;
            self.entries.insert(rel, entry);
        }
        Ok(())
    }

    pub fn remove(&mut self, disk: &str) {
        self.dirty = true;
        self.entries.remove(disk);
    }

    /// フォルダ(ディスク上の相対パス)以下のページを索引から外す。
    pub fn remove_dir(&mut self, dir: &str) {
        self.dirty = true;
        let prefix = format!("{dir}/");
        self.entries.retain(|disk, _| !disk.starts_with(&prefix));
    }

    pub fn meta(&self, disk: &str) -> Option<PageMeta> {
        self.entries.get(disk).map(Entry::meta)
    }

    /// 変わっていれば保存する。保存できなくても索引は使えるので、失敗は無視する。
    pub fn save(&mut self) {
        if !self.dirty {
            return;
        }
        let bytes = encode(&self.entries);
        let Some(dir) = self.file.parent() else {
            return;
        };
        if fs::create_dir_all(dir).is_ok() && disk::replace_file(&self.file, &bytes, None).is_ok() {
            self.dirty = false;
        }
    }

    pub fn list_tree(&self) -> Vec<TreeNode> {
        let mut root = DirBuild::default();
        for entry in self.entries.values() {
            let mut dir = &mut root;
            let mut parts: Vec<&str> = entry.disk.split('/').collect();
            let name = parts.pop().unwrap_or_default();
            for part in parts {
                dir = dir.dirs.entry(key(part)).or_insert_with(|| DirBuild {
                    name: part.to_string(),
                    ..Default::default()
                });
            }
            dir.pages.push(name.to_string());
        }
        root.nodes("")
    }

    pub fn recent_pages(&self, limit: usize) -> Vec<PageMeta> {
        let mut entries: Vec<&Entry> = self.entries.values().collect();
        entries.sort_by(|a, b| newest_first(a, b));
        entries.into_iter().take(limit).map(Entry::meta).collect()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let words = query_words(query);
        if words.is_empty() || limit == 0 {
            return Vec::new();
        }
        // (タイトルにすべての語があるか, ページ)
        let mut matched: Vec<(bool, &Entry)> = Vec::new();
        for entry in self.entries.values() {
            let Some(text) = &entry.text else {
                continue;
            };
            let found = words.iter().all(|w| {
                entry.folded_title.contains(w.as_str()) || text.folded.contains(w.as_str())
            });
            if found {
                let title_matched = words
                    .iter()
                    .all(|w| entry.folded_title.contains(w.as_str()));
                matched.push((title_matched, entry));
            }
        }
        matched.sort_by(|(ta, a), (tb, b)| tb.cmp(ta).then_with(|| newest_first(a, b)));
        matched
            .into_iter()
            .take(limit)
            .map(|(title_matched, entry)| {
                let text = entry.text.as_ref().expect("UTF-8 のページだけを集めた");
                let in_body: Vec<String> = words
                    .iter()
                    .filter(|w| text.folded.contains(w.as_str()))
                    .cloned()
                    .collect();
                SearchHit {
                    path: entry.path.clone(),
                    title: entry.title.clone(),
                    snippet: snippet(&text.body, &in_body),
                    title_matched,
                }
            })
            .collect()
    }
}

/// 更新日時の新しい順、同じなら path の自然な順。
fn newest_first(a: &Entry, b: &Entry) -> Ordering {
    b.modified_at
        .cmp(&a.modified_at)
        .then_with(|| natural_path_cmp(&a.path, &b.path))
}

/// list_tree を組み立てるための、フォルダ1つ分。
#[derive(Default)]
struct DirBuild {
    /// ディスク上の名前
    name: String,
    /// ページのファイル名(ディスク上の表記)
    pages: Vec<String>,
    /// 比べるための形 → 子フォルダ
    dirs: BTreeMap<String, DirBuild>,
}

impl DirBuild {
    /// このフォルダの中身を節点にする。`prefix` は、このフォルダの path(NFC)の後ろに「/」を付けたもの。
    fn nodes(mut self, prefix: &str) -> Vec<TreeNode> {
        let mut nodes = Vec::new();
        for page in std::mem::take(&mut self.pages) {
            let title = nfc(stem(&page));
            // ページと同じ名前のフォルダは、そのページの子になる。
            let children = match self.dirs.remove(&key(&title)) {
                Some(dir) => {
                    let dir_prefix = format!("{prefix}{}/", nfc(&dir.name));
                    dir.nodes(&dir_prefix)
                }
                None => Vec::new(),
            };
            nodes.push(TreeNode {
                kind: NodeKind::Page,
                path: format!("{prefix}{}", nfc(&page)),
                title,
                children,
            });
        }
        for dir in std::mem::take(&mut self.dirs).into_values() {
            let name = nfc(&dir.name);
            let path = format!("{prefix}{name}");
            let children = dir.nodes(&format!("{path}/"));
            nodes.push(TreeNode {
                kind: NodeKind::Folder,
                path,
                title: name,
                children,
            });
        }
        nodes.sort_by(|a, b| natural_cmp(&a.title, &b.title));
        nodes
    }
}

// ---------------------------------------------------------------- 保存の形式
//
// MAGIC、ページの数、ページごとの (disk, size, mtime_ns, modified_at, 本文があるか, 本文, 比べるための形)、
// 最後にここまでの SHA-256。数はすべてリトルエンディアン。

fn put_u64(out: &mut Vec<u8>, n: u64) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u64(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

fn encode(entries: &BTreeMap<String, Entry>) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut out = MAGIC.to_vec();
    put_u64(&mut out, entries.len() as u64);
    for e in entries.values() {
        put_str(&mut out, &e.disk);
        put_u64(&mut out, e.size);
        out.extend_from_slice(&e.mtime_ns.to_le_bytes());
        put_u64(&mut out, e.modified_at);
        match &e.text {
            Some(text) => {
                out.push(1);
                put_str(&mut out, &text.body);
                put_str(&mut out, &text.folded);
            }
            None => out.push(0),
        }
    }
    let hash = Sha256::digest(&out);
    out.extend_from_slice(&hash);
    out
}

/// 読み込んだバイト列を索引に戻す。形が少しでもおかしければ None。
fn decode(bytes: &[u8]) -> Option<BTreeMap<String, Entry>> {
    use sha2::{Digest, Sha256};
    let (payload, hash) = bytes.split_at_checked(bytes.len().checked_sub(32)?)?;
    if Sha256::digest(payload).as_slice() != hash {
        return None;
    }
    let mut r = Reader(payload.strip_prefix(MAGIC.as_slice())?);
    let count = r.u64()?;
    let mut entries = BTreeMap::new();
    for _ in 0..count {
        let disk = r.string()?;
        if !crate::names::is_md_name(&disk) {
            return None;
        }
        let size = r.u64()?;
        let mtime_ns = u128::from_le_bytes(r.take(16)?.try_into().ok()?);
        let modified_at = r.u64()?;
        let text = match r.take(1)?[0] {
            0 => None,
            1 => Some(Text {
                body: r.string()?,
                folded: r.string()?,
            }),
            _ => return None,
        };
        let path = nfc(&disk);
        let title = stem(path.rsplit('/').next()?).to_string();
        let entry = Entry {
            folded_title: fold(&title),
            path,
            title,
            disk: disk.clone(),
            size,
            mtime_ns,
            modified_at,
            text,
        };
        entries.insert(disk, entry);
    }
    r.0.is_empty().then_some(entries)
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (head, rest) = self.0.split_at_checked(n)?;
        self.0 = rest;
        Some(head)
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn string(&mut self) -> Option<String> {
        let len = usize::try_from(self.u64()?).ok()?;
        String::from_utf8(self.take(len)?.to_vec()).ok()
    }
}
