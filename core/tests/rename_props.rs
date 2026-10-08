//! 段階4の性質テスト(proptest)

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use common::*;
use proptest::prelude::*;

/// vault 内のすべてのファイルの中身(フォルダは空の中身として記録する)。
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    all_entries(root)
        .into_iter()
        .map(|rel| {
            let bytes = if rel.ends_with('/') {
                Vec::new()
            } else {
                fs::read(root.join(&rel)).unwrap()
            };
            (rel, bytes)
        })
        .collect()
}

/// ページ本文の断片。リンクの行き先だけが、名前を変えるページの名前に依存する。
#[derive(Debug, Clone)]
enum Piece {
    /// 名前を変えるページへのリンク(書き換わる)
    Link,
    /// その子ページへのリンク(書き換わる)
    ChildLink,
    /// 見出しつきのリンク(書き換わる)
    LinkWithFragment,
    /// インラインコードの中のリンク風の文字列(書き換わらない)
    InlineCode,
    /// コードブロックの中のリンク風の文字列(書き換わらない)
    Fenced,
    /// 別のページへのリンク(書き換わらない)
    OtherLink,
    /// リンクではないファイル名(書き換わらない)
    PlainName,
    Text(String),
}

impl Piece {
    /// `up` はそのページからルートへの前置き、`name` は名前を変えるページの今のタイトル。
    fn render(&self, up: &str, name: &str) -> String {
        match self {
            Piece::Link => format!("[リンク]({up}{name}.md)"),
            Piece::ChildLink => format!("[子へ]({up}{name}/c.md)"),
            Piece::LinkWithFragment => format!("[見出し]({up}{name}.md#議題)"),
            Piece::InlineCode => format!("`[コード]({up}a.md)`"),
            Piece::Fenced => format!("\n```\n[例]({up}a.md)\n```\n"),
            Piece::OtherLink => format!("[別]({up}x.md)"),
            Piece::PlainName => "a.md".to_string(),
            Piece::Text(s) => s.clone(),
        }
    }
}

type Body = Vec<(Piece, &'static str)>;

fn body() -> impl Strategy<Value = Body> {
    let piece = prop_oneof![
        Just(Piece::Link),
        Just(Piece::ChildLink),
        Just(Piece::LinkWithFragment),
        Just(Piece::InlineCode),
        Just(Piece::Fenced),
        Just(Piece::OtherLink),
        Just(Piece::PlainName),
        "[あ-んA-Za-z ]{0,6}".prop_map(Piece::Text),
    ];
    let sep = prop_oneof![Just(" "), Just("\n"), Just("\r\n"), Just("")];
    prop::collection::vec((piece, sep), 0..8)
}

fn render(body: &Body, up: &str, name: &str) -> String {
    body.iter()
        .map(|(p, sep)| format!("{}{sep}", p.render(up, name)))
        .collect()
}

const NEW_TITLE: &str = "新[ぁ-んa-z0-9]{0,8}";

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// rename の後、どのページもリンクの行き先(と、旧タイトルと同じ表示文字列)だけが変わり、
    /// ほかは1バイトも変わらない。改名するページ自身と、その子ページの中も同じ。
    #[test]
    fn rename_changes_only_link_destinations(
        self_body in body(),
        child_body in body(),
        root_body in body(),
        nested_body in body(),
        new_title in NEW_TITLE,
    ) {
        let v = VaultBuilder::new()
            .file("a.md", render(&self_body, "", "a"))
            .file("a/c.md", render(&child_body, "../", "a"))
            .file("x.md", render(&root_body, "", "a"))
            .file("y/z.md", render(&nested_body, "../", "a"))
            .open();

        v.rename_page("a.md", &new_title).unwrap();

        let read = |rel: String| String::from_utf8(v.disk_bytes(&rel)).unwrap();
        prop_assert_eq!(read(format!("{new_title}.md")), render(&self_body, "", &new_title));
        prop_assert_eq!(read(format!("{new_title}/c.md")), render(&child_body, "../", &new_title));
        prop_assert_eq!(read("x.md".into()), render(&root_body, "", &new_title));
        prop_assert_eq!(read("y/z.md".into()), render(&nested_body, "../", &new_title));
    }

    /// rename してから元の名前に戻すと、vault のすべてのファイルがバイト単位で元通りになる。
    #[test]
    fn rename_and_back_restores_every_file(
        self_body in body(),
        root_body in body(),
        nested_body in body(),
        child_body in body(),
        new_title in NEW_TITLE,
    ) {
        let v = VaultBuilder::new()
            .file("a.md", render(&self_body, "", "a"))
            .file("a/c.md", render(&child_body, "../", "a"))
            .file("x.md", render(&root_body, "", "a"))
            .file("y/z.md", render(&nested_body, "../", "a"))
            .open();
        let before = snapshot(&v.root());

        let meta = v.rename_page("a.md", &new_title).unwrap();
        v.rename_page(&meta.path, "a").unwrap();

        prop_assert_eq!(snapshot(&v.root()), before);
    }

    /// どんな新しいタイトルでも、書き換えたリンクを GFM として解釈すると、
    /// 表示文字列は新しいタイトルに、行き先は新しいファイル名になる。表の中でも列の数は変わらない。
    /// (タイトルの処理で置き換わる / : \\ は含めない。ほかの ASCII の記号はすべて含める。)
    #[test]
    fn rewritten_link_reads_back_as_new_title(
        new_title in r##"新[a-zあ-ん0-9 !"#$%&'()*+,\-.;<=>?@\[\]^_`{|}~\x{3000}\x{A0}]{0,12}[a-zあ-ん!"#$%&'()*+,\-.;<=>?@\[\]^_`{|}~]"##,
    ) {
        let v = VaultBuilder::new()
            .file("a.md", "")
            .file("other.md", "前の文 [a](a.md) 後の文\n")
            .file("table.md", "| 列1 | 列2 |\n|---|---|\n| [a](a.md) | x |\n")
            .open();

        let meta = v.rename_page("a.md", &new_title).unwrap();

        prop_assert_eq!(&meta.title, &new_title);
        let expected = [(new_title.clone(), format!("{new_title}.md"))];
        let other = String::from_utf8(v.disk_bytes("other.md")).unwrap();
        prop_assert_eq!(links_in(&other), expected.clone());
        let table = String::from_utf8(v.disk_bytes("table.md")).unwrap();
        prop_assert_eq!(table_shape(&table), [2, 2], "{:?}", table);
        prop_assert_eq!(links_in(&table), expected);
    }
}
