//! テスト用の補助関数。vault を数行で組み立てられるようにする。
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use odin_core::{Trash, TreeNode, Vault};
use tempfile::TempDir;
use unicode_normalization::UnicodeNormalization;

/// 一時ディレクトリの中に `vault/` を作り、そこを vault のルートにする。
/// ルートの外(一時ディレクトリ直下)にもファイルを置けるので、
/// 「vault の外を読めないこと」を確かめられる。
/// 索引用のフォルダは、これとは別の一時ディレクトリに置く。
pub struct VaultBuilder {
    tmp: TempDir,
    index: TempDir,
    trash_fails_on: Option<usize>,
}

/// テスト用のゴミ箱。OS のゴミ箱を汚さないよう、一時ディレクトリへ実際に移動する。
/// 呼ばれるたびに番号つきのフォルダを作り、その中へ元の名前のまま移す。
pub struct DirTrash {
    dir: PathBuf,
    calls: Arc<AtomicUsize>,
    /// この回数目(1から数える)の呼び出しだけ、何も移さずに失敗する。
    fails_on: Option<usize>,
}

impl Trash for DirTrash {
    fn trash(&self, path: &Path) -> std::io::Result<()> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fails_on == Some(n + 1) {
            return Err(std::io::Error::other("DirTrash: 指定どおりに失敗"));
        }
        let slot = self.dir.join(n.to_string());
        fs::create_dir(&slot)?;
        fs::rename(path, slot.join(path.file_name().unwrap()))
    }
}

impl VaultBuilder {
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("vault")).unwrap();
        fs::create_dir(tmp.path().join("trash")).unwrap();
        Self {
            tmp,
            index: tempfile::tempdir().unwrap(),
            trash_fails_on: None,
        }
    }

    pub fn root(&self) -> PathBuf {
        self.tmp.path().join("vault")
    }

    pub fn index_dir(&self) -> PathBuf {
        self.index.path().to_path_buf()
    }

    /// vault 内に、親フォルダごとファイルを作る。内容はバイト列のまま書く。
    pub fn file(self, rel: &str, content: impl AsRef<[u8]>) -> Self {
        let p = self.root().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
        self
    }

    /// 開く前に、ファイルの mtime を設定する。
    pub fn mtime(self, rel: &str, t: SystemTime) -> Self {
        set_mtime(&self.root().join(rel), t);
        self
    }

    pub fn dir(self, rel: &str) -> Self {
        fs::create_dir_all(self.root().join(rel)).unwrap();
        self
    }

    /// vault 内に、`target` を指すシンボリックリンク `rel` を作る。
    pub fn symlink(self, rel: &str, target: &Path) -> Self {
        std::os::unix::fs::symlink(target, self.root().join(rel)).unwrap();
        self
    }

    /// vault の外(ルートの1つ上)にファイルを作る。
    pub fn outside_file(self, name: &str, content: &str) -> Self {
        fs::write(self.outside(name), content).unwrap();
        self
    }

    pub fn outside(&self, name: &str) -> PathBuf {
        self.tmp.path().join(name)
    }

    /// ゴミ箱への n 回目(1から数える)の移動を失敗させる。
    pub fn trash_fails_on_call(mut self, n: usize) -> Self {
        self.trash_fails_on = Some(n);
        self
    }

    /// テスト用のゴミ箱(`DirTrash`)を使って開く。
    pub fn open(self) -> TestVault {
        let trash = DirTrash {
            dir: self.tmp.path().join("trash"),
            calls: Arc::default(),
            fails_on: self.trash_fails_on,
        };
        let vault =
            Vault::open_with_trash(&self.root(), &self.index_dir(), Box::new(trash)).unwrap();
        TestVault {
            tmp: self.tmp,
            index: self.index,
            vault: Some(vault),
        }
    }
}

pub struct TestVault {
    tmp: TempDir,
    index: TempDir,
    /// 開き直す(`reopen`)ときに、いったん閉じてから開けるよう Option にしている。
    vault: Option<Vault>,
}

impl Deref for TestVault {
    type Target = Vault;
    fn deref(&self) -> &Vault {
        self.vault.as_ref().unwrap()
    }
}

impl TestVault {
    pub fn index_dir(&self) -> PathBuf {
        self.index.path().to_path_buf()
    }

    /// いったん閉じてから、同じ vault と index_dir で開き直す(アプリの再起動にあたる)。
    pub fn reopen(&mut self) {
        drop(self.vault.take());
        let trash = DirTrash {
            dir: self.tmp.path().join("trash"),
            // 番号のフォルダが重ならないよう、これまでに移した数から数え始める。
            calls: Arc::new(AtomicUsize::new(
                fs::read_dir(self.tmp.path().join("trash")).unwrap().count(),
            )),
            fails_on: None,
        };
        let vault =
            Vault::open_with_trash(&self.root(), &self.index_dir(), Box::new(trash)).unwrap();
        self.vault = Some(vault);
    }

    /// 閉じた状態で索引用のフォルダの中身をすべて消し、開き直す。
    pub fn delete_index_and_reopen(&mut self) {
        let index_dir = self.index_dir();
        self.delete_index_and_reopen_with(|| {
            for e in fs::read_dir(&index_dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    fs::remove_dir_all(p).unwrap()
                } else {
                    fs::remove_file(p).unwrap()
                }
            }
        });
    }

    /// 閉じた状態で `delete` を実行してから、開き直す。
    pub fn delete_index_and_reopen_with(&mut self, delete: impl FnOnce()) {
        drop(self.vault.take());
        delete();
        self.reopen();
    }

    pub fn root(&self) -> PathBuf {
        self.tmp.path().join("vault")
    }

    pub fn outside(&self, name: &str) -> PathBuf {
        self.tmp.path().join(name)
    }

    pub fn disk_bytes(&self, rel: &str) -> Vec<u8> {
        fs::read(self.root().join(rel)).unwrap()
    }

    /// アプリの外からファイルを書き換える(親フォルダも作る)。
    pub fn write_outside_app(&self, rel: &str, content: impl AsRef<[u8]>) {
        let p = self.root().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    pub fn set_mtime(&self, rel: &str, t: SystemTime) {
        set_mtime(&self.root().join(rel), t);
    }

    /// vault 内にある .md 以外のファイルとフォルダの一覧(隠しファイルも含む)。
    pub fn non_md_entries(&self) -> BTreeSet<String> {
        non_md_entries(&self.root())
    }
}

pub fn non_md_entries(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    fn walk(root: &Path, dir: &Path, out: &mut BTreeSet<String>) {
        for e in fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            let ft = e.file_type().unwrap();
            if ft.is_dir() {
                out.insert(format!("{rel}/"));
                walk(root, &p, out);
            } else if !rel.to_lowercase().ends_with(".md") {
                out.insert(rel);
            }
        }
    }
    walk(root, root, &mut out);
    out
}

pub fn set_mtime(path: &Path, t: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

pub fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

/// 木に含まれるすべての節点の path を集める(順序は問わない)。
pub fn all_paths(tree: &[TreeNode]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    fn walk(nodes: &[TreeNode], out: &mut BTreeSet<String>) {
        for n in nodes {
            out.insert(n.path.clone());
            walk(&n.children, out);
        }
    }
    walk(tree, &mut out);
    out
}

/// 同じ階層の節点のタイトルを、返ってきた順に並べる。
pub fn titles(nodes: &[TreeNode]) -> Vec<&str> {
    nodes.iter().map(|n| n.title.as_str()).collect()
}

/// 木から path が完全一致する節点を探す。
pub fn find<'a>(tree: &'a [TreeNode], path: &str) -> Option<&'a TreeNode> {
    for n in tree {
        if n.path == path {
            return Some(n);
        }
        if let Some(found) = find(&n.children, path) {
            return Some(found);
        }
    }
    None
}

pub fn nfc(s: &str) -> String {
    s.nfc().collect()
}

pub fn nfd(s: &str) -> String {
    s.nfd().collect()
}

/// `root` 以下のすべてのファイルとフォルダ(隠しファイルも含む)。フォルダは末尾に `/` を付ける。
/// 名前はディスク上の表記のまま(NFC にそろえない)。
pub fn all_entries(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    fn walk(root: &Path, dir: &Path, out: &mut BTreeSet<String>) {
        for e in fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            if e.file_type().unwrap().is_dir() {
                out.insert(format!("{rel}/"));
                walk(root, &p, out);
            } else {
                out.insert(rel);
            }
        }
    }
    walk(root, root, &mut out);
    out
}

/// フォルダ直下にある名前を、ディスク上の表記のまま返す。
pub fn names_on_disk(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect()
}

impl TestVault {
    pub fn all_entries(&self) -> BTreeSet<String> {
        all_entries(&self.root())
    }

    pub fn version(&self, path: &str) -> String {
        self.read_page(path).unwrap().1
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.root().join(rel).symlink_metadata().is_ok()
    }

    /// ゴミ箱に移されたものの一覧。番号のフォルダを除いた、元の名前からの相対パス。
    /// フォルダは末尾に `/` を付ける。
    pub fn trashed(&self) -> BTreeSet<String> {
        let trash = self.tmp.path().join("trash");
        let mut out = BTreeSet::new();
        for slot in fs::read_dir(&trash).unwrap() {
            out.extend(all_entries(&slot.unwrap().path()));
        }
        out
    }

    /// ゴミ箱に移されたファイルの中身。`trashed()` と同じ相対パスで指定する。
    pub fn trashed_bytes(&self, rel: &str) -> Vec<u8> {
        let trash = self.tmp.path().join("trash");
        for slot in fs::read_dir(&trash).unwrap() {
            if let Ok(b) = fs::read(slot.unwrap().path().join(rel)) {
                return b;
            }
        }
        panic!("ゴミ箱に {rel} がない");
    }

    pub fn mode(&self, rel: &str) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(self.root().join(rel))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777
    }

    pub fn set_mode(&self, rel: &str, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(self.root().join(rel), fs::Permissions::from_mode(mode)).unwrap();
    }

    pub fn created(&self, rel: &str) -> SystemTime {
        fs::metadata(self.root().join(rel))
            .unwrap()
            .created()
            .unwrap()
    }

    /// 修正時刻をミリ秒の UNIX 時刻で返す。
    pub fn mtime_ms(&self, rel: &str) -> u64 {
        mtime(&self.root().join(rel))
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}
