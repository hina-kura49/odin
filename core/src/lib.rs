//! メモアプリのバックエンドの中核。Tauri に依存しない。
//!
//! この段階ではテストをコンパイルさせるための骨組みだけを置いている。
//! 関数の本体はすべて `todo!()` で、実装は担当者が書く。

use std::path::{Path, PathBuf};

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
        let _ = path;
        todo!()
    }
}

pub struct Vault {
    // vault のフォルダ
    root: PathBuf,
    // ページを消すときに使うゴミ箱。
    trash: Box<dyn Trash>,
}

impl Vault {
    /// `index_dir` は vault の外に置く索引用のフォルダ。
    /// 存在しなければ途中も含めて作る。
    pub fn open(root: &Path, index_dir: &Path) -> Result<Vault> {
        Vault::open_with_trash(root, index_dir, Box::new(SystemTrash))
    }

    /// ゴミ箱の処理を差し替えて開く。それ以外は`open`と同じ。
    pub fn open_with_trash(root: &Path, index_dir: &Path, trash: Box<dyn Trash>) -> Result<Vault> {
        // index_dirハステップ１２で使う。今は使わない。
        let _ = index_dir;

        // rootがフォルダでなければエラーにする。
        if !root.is_dir() {
            return Err(Error::NotFound(root.display().to_string()));
        }

        Ok(Vault {
            root: root.to_path_buf(),
            trash,
        })
    }

    pub fn list_tree(&self) -> Result<Vec<TreeNode>> {
        todo!()
    }
    /// 内容と version を返す。version は内容のハッシュに基づく不透明な文字列。
    pub fn read_page(&self, path: &str) -> Result<(String, String)> {
        // vaultのフォルダとpathをつなげて、ファイルの場所を作る。
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

        // バイト列を文字列にする。UTF-8として読めなければNotUtf8にする。
        let content = match String::from_utf8(bytes) {
            Ok(content) => content,
            Err(_) => return Err(Error::NotUtf8(path.to_string())),
        };

        // versionはステップ４で作る。今は仮に空にしておく。
        let version = String::new();

        Ok((content, version))
    }

    /// ディスク上の現在の内容の version が `base_version` と異なれば `Conflict` を返し、
    /// ファイルを変更しない。書き込みは一時ファイルに書いてから rename する。
    pub fn write_page(&self, path: &str, content: &str, base_version: &str) -> Result<WriteResult> {
        let _ = (path, content, base_version);
        todo!()
    }

    /// `parent` はページの path か Folder の節点の path。
    /// ページなら子はそれと同名のフォルダの中に、Folder ならそのフォルダの中に作る。
    /// 新しいページの中身は空(0バイト)。
    pub fn create_page(&self, parent: Option<&str>, title: &str) -> Result<PageMeta> {
        let _ = (parent, title);
        todo!()
    }

    /// 子フォルダ(中身を問わない)を先に、ページを後にゴミ箱へ移す。元に戻す処理はしない。
    /// その結果、親のフォルダが空になったら、ルートを除いて上へ順に削除する。
    pub fn delete_page(&self, path: &str) -> Result<()> {
        let _ = path;
        todo!()
    }

    /// ファイル名を変え、子フォルダがあれば一緒に変え、vault 内の他のページからのリンクを書き換える。
    /// リンク以外の本文は1バイトも変えない。
    pub fn rename_page(&self, path: &str, new_title: &str) -> Result<PageMeta> {
        let _ = (path, new_title);
        todo!()
    }

    /// タイトルと本文(ファイルの全文)から探し、並び順の上位 `limit` 件を返す。
    /// 比較は NFKC にそろえ、英字の大文字小文字を区別しない。空白で区切った語はすべてを含むページを探す。
    /// 並び順: タイトル一致 > 本文だけの一致、同じ順位では modified_at の新しい順、次に path 順。
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let _ = (query, limit);
        todo!()
    }

    /// 更新日時(modified_at)の新しい順(同じなら path 順)に、最大 `limit` 件を返す。
    pub fn recent_pages(&self, limit: usize) -> Result<Vec<PageMeta>> {
        let _ = limit;
        todo!()
    }

    /// vault のファイルから索引を作り直す。
    pub fn rebuild_index(&self) -> Result<()> {
        todo!()
    }

    /// アプリの外での追加・変更・削除を、list_tree・search・recent_pages に反映する。
    /// 大きさと mtime が同じで中身だけ違う変更は見逃してよい(rebuild_index では反映される)。
    pub fn rescan(&self) -> Result<()> {
        todo!()
    }

    /// vault 直下の `Inbox.md` の末尾に、空行を挟んで追記する。なければ作る。
    pub fn capture_to_inbox(&self, text: &str) -> Result<()> {
        let _ = text;
        todo!()
    }
}
