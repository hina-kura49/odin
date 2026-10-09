//! フロントエンドとの境界を越える型。
//!
//! core の型は serde を持たない(core は変更しない)ので、ここで同じ形の型を持ち、core の型から変換する。
//! - 変換では core の型の欄をすべて名指しで取り出す(`..` を使わない)。core に欄や種類が増えたら、ここがコンパイルできなくなる
//! - TypeScript の型は、ここから ts-rs で src/backend/generated/ に書き出す(npm run gen:types)。手では書かない
//! - 契約(src/backend/types.ts)と形が違うものは、ここで契約に合わせず、TauriBackend の境目で変換する

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum NodeKind {
    Page,
    Folder,
}

impl From<odin_core::NodeKind> for NodeKind {
    fn from(kind: odin_core::NodeKind) -> Self {
        match kind {
            odin_core::NodeKind::Page => NodeKind::Page,
            odin_core::NodeKind::Folder => NodeKind::Folder,
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TreeNode {
    pub path: String,
    pub title: String,
    pub kind: NodeKind,
    pub children: Vec<TreeNode>,
}

impl From<odin_core::TreeNode> for TreeNode {
    fn from(node: odin_core::TreeNode) -> Self {
        let odin_core::TreeNode { kind, path, title, children } = node;
        TreeNode { path, title, kind: kind.into(), children: children.into_iter().map(Into::into).collect() }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PageMeta {
    pub path: String,
    pub title: String,
    /// UNIX 時刻のミリ秒
    pub modified_at: u64,
}

impl From<odin_core::PageMeta> for PageMeta {
    fn from(meta: odin_core::PageMeta) -> Self {
        let odin_core::PageMeta { path, title, modified_at } = meta;
        PageMeta { path, title, modified_at }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Snippet {
    pub before: String,
    pub hit: String,
    pub after: String,
}

impl From<odin_core::Snippet> for Snippet {
    fn from(snippet: odin_core::Snippet) -> Self {
        let odin_core::Snippet { before, hit, after } = snippet;
        Snippet { before, hit, after }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub title_matched: bool,
    pub snippet: Snippet,
}

impl From<odin_core::SearchHit> for SearchHit {
    fn from(hit: odin_core::SearchHit) -> Self {
        let odin_core::SearchHit { path, title, snippet, title_matched } = hit;
        SearchHit { path, title, title_matched, snippet: snippet.into() }
    }
}

/// read_page の結果(core は (内容, version) の組で返す)
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PageContent {
    pub content: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export)]
pub enum WriteResult {
    Ok { version: String },
    Conflict,
}

impl From<odin_core::WriteResult> for WriteResult {
    fn from(result: odin_core::WriteResult) -> Self {
        match result {
            odin_core::WriteResult::Ok { version } => WriteResult::Ok { version },
            odin_core::WriteResult::Conflict => WriteResult::Conflict,
        }
    }
}

/// エラーの種類。core の Error の種類と1対1に対応させる
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ErrorKind {
    NotFound,
    InvalidPath,
    NotAPage,
    NotUtf8,
    ReadOnly,
    NameOccupied,
    IndexOverlapsVault,
    Io,
}

/// コマンドが失敗したときにフロントへ返すもの(フロントの toBackendError がこの形を読む)
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct IpcError {
    pub kind: ErrorKind,
    pub message: String,
}

impl IpcError {
    pub fn io(message: impl Into<String>) -> Self {
        IpcError { kind: ErrorKind::Io, message: message.into() }
    }
}

impl From<odin_core::Error> for IpcError {
    fn from(error: odin_core::Error) -> Self {
        let message = error.to_string();
        // core の Error は #[non_exhaustive] なので、ほかのクレートからは `_` の腕が必須になる(種類が増えてもここではコンパイルが通ってしまう)。
        // そのため build.rs で、core/src/lib.rs の種類と、この match で名指ししている種類を突き合わせ、食い違えばビルドを止める。
        let kind = match error {
            odin_core::Error::NotFound(_) => ErrorKind::NotFound,
            odin_core::Error::InvalidPath(_) => ErrorKind::InvalidPath,
            odin_core::Error::NotAPage(_) => ErrorKind::NotAPage,
            odin_core::Error::NotUtf8(_) => ErrorKind::NotUtf8,
            odin_core::Error::ReadOnly(_) => ErrorKind::ReadOnly,
            odin_core::Error::NameOccupied(_) => ErrorKind::NameOccupied,
            odin_core::Error::IndexOverlapsVault(_) => ErrorKind::IndexOverlapsVault,
            odin_core::Error::Io(_) => ErrorKind::Io,
            _ => ErrorKind::Io,
        };
        IpcError { kind, message }
    }
}

