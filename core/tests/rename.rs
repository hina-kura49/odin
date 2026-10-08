//! 段階4: rename_page(ファイル名・子フォルダ・他ページのリンクの書き換え)

mod common;

use std::time::{Duration, SystemTime};

use common::*;
use odin_core::{Error, NodeKind, PageMeta, Vault};

fn rename(v: &Vault, path: &str, new_title: &str) -> PageMeta {
    v.rename_page(path, new_title).unwrap()
}

fn text(v: &TestVault, rel: &str) -> String {
    String::from_utf8(v.disk_bytes(rel)).unwrap()
}

fn t(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

// ================================================================ ファイル名と子フォルダ

#[test]
fn rename_page_renames_the_file() {
    let v = VaultBuilder::new().file("会議.md", "本文").open();

    rename(&v, "会議.md", "定例会議");

    assert_eq!(v.all_entries(), ["定例会議.md".to_string()].into());
}

#[test]
fn rename_page_keeps_content_of_renamed_page() {
    let content = "\u{FEFF}---\ntitle: 会議\n---\r\n本文  \n末尾改行なし";
    let v = VaultBuilder::new().file("会議.md", content).open();

    rename(&v, "会議.md", "定例会議");

    assert_eq!(v.disk_bytes("定例会議.md"), content.as_bytes());
}

#[test]
fn rename_page_returns_new_path_and_title() {
    let v = VaultBuilder::new().file("a.md", "").open();

    let meta = rename(&v, "a.md", "b");

    assert_eq!(meta.path, "b.md");
    assert_eq!(meta.title, "b");
}

#[test]
fn rename_page_returns_modified_at_as_file_mtime_in_milliseconds() {
    let v = VaultBuilder::new().file("a.md", "").open();

    let meta = rename(&v, "a.md", "b");

    assert_eq!(meta.modified_at, v.mtime_ms("b.md"));
}

#[test]
fn renamed_page_is_readable_at_new_path_with_same_version() {
    let v = VaultBuilder::new().file("a.md", "本文").open();
    let before = v.read_page("a.md").unwrap();

    let meta = rename(&v, "a.md", "b");

    assert_eq!(v.read_page(&meta.path).unwrap(), before);
}

#[test]
fn read_page_returns_not_found_for_old_path_after_rename() {
    let v = VaultBuilder::new().file("a.md", "").open();
    rename(&v, "a.md", "b");

    let err = v.read_page("a.md").unwrap_err();

    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

#[test]
fn renamed_page_appears_in_list_tree_at_new_path() {
    let v = VaultBuilder::new().file("a.md", "").file("z.md", "").open();

    rename(&v, "a.md", "b");

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["b.md", "z.md"].map(String::from).into()
    );
}

#[test]
fn rename_page_renames_child_folder_together() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", "子")
        .file("a/c/d.md", "孫")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        v.all_entries(),
        ["b.md", "b/", "b/c.md", "b/c/", "b/c/d.md"]
            .map(String::from)
            .into()
    );
    assert_eq!(v.disk_bytes("b/c/d.md"), "孫".as_bytes());
}

#[test]
fn rename_page_moves_everything_in_child_folder() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", "")
        .file("a/画像.png", "png")
        .file("a/.DS_Store", "x")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        v.all_entries(),
        ["b.md", "b/", "b/c.md", "b/画像.png", "b/.DS_Store"]
            .map(String::from)
            .into()
    );
}

#[test]
fn renamed_page_keeps_its_children_in_list_tree() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", "")
        .open();

    rename(&v, "a.md", "b");

    let tree = v.list_tree().unwrap();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Page);
    assert_eq!(all_paths(&tree[0].children), ["b/c.md".to_string()].into());
}

#[test]
fn rename_page_renames_nested_page_and_its_children() {
    let v = VaultBuilder::new()
        .file("p.md", "")
        .file("p/a.md", "")
        .file("p/a/c.md", "")
        .open();

    let meta = rename(&v, "p/a.md", "b");

    assert_eq!(meta.path, "p/b.md");
    assert_eq!(
        v.all_entries(),
        ["p.md", "p/", "p/b.md", "p/b/", "p/b/c.md"]
            .map(String::from)
            .into()
    );
}

#[test]
fn rename_page_handles_japanese_emoji_and_mixed_width_titles() {
    let v = VaultBuilder::new().file("会議.md", "").open();

    let meta = rename(&v, "会議.md", "定例📝ＡＢＣabc");

    assert_eq!(meta.path, "定例📝ＡＢＣabc.md");
}

#[test]
fn rename_page_accepts_path_differing_in_case_and_normalization() {
    let v = VaultBuilder::new()
        .file(&format!("{}.md", nfd("Gakkoがっこう")), "")
        .open();

    let meta = rename(&v, &nfc("gakkoがっこう.md"), "b");

    assert_eq!(meta.path, "b.md");
    assert_eq!(v.all_entries(), ["b.md".to_string()].into());
}

// ================================================================ リンクの書き換え

#[test]
fn rename_page_rewrites_link_in_other_page() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "前の文 [こちら](a.md) 後の文\n")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "前の文 [こちら](b.md) 後の文\n");
}

#[test]
fn rename_page_rewrites_every_link_in_a_page() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[1](a.md) と [2](a.md)\n\n- [3](a.md)\n")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        text(&v, "other.md"),
        "[1](b.md) と [2](b.md)\n\n- [3](b.md)\n"
    );
}

#[test]
fn rename_page_rewrites_links_in_every_page() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("x.md", "[x](a.md)")
        .file("y.md", "[y](a.md)")
        .file("z/w.md", "[w](../a.md)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "x.md"), "[x](b.md)");
    assert_eq!(text(&v, "y.md"), "[y](b.md)");
    assert_eq!(text(&v, "z/w.md"), "[w](../b.md)");
}

#[test]
fn rename_page_rewrites_relative_link_from_page_in_other_folder() {
    let v = VaultBuilder::new()
        .file("p.md", "")
        .file("p/a.md", "")
        .file("q/r/s.md", "[a](../../p/a.md)")
        .open();

    rename(&v, "p/a.md", "b");

    assert_eq!(text(&v, "q/r/s.md"), "[a](../../p/b.md)");
}

#[test]
fn rename_page_rewrites_links_to_its_child_pages() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", "")
        .file("a/c/d.md", "")
        .file("other.md", "[c](a/c.md) [d](a/c/d.md)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "[c](b/c.md) [d](b/c/d.md)");
}

#[test]
fn rename_page_rewrites_link_from_child_to_renamed_parent() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", "[親](../a.md)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "b/c.md"), "[親](../b.md)");
}

#[test]
fn rename_page_keeps_links_between_children_unchanged() {
    let content = "[兄弟](d.md) [孫](d/e.md)";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", content)
        .file("a/d.md", "")
        .file("a/d/e.md", "")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "b/c.md"), content);
}

#[test]
fn rename_page_keeps_links_from_children_to_outside_pages_unchanged() {
    let content = "[外](../x.md) [もっと外](../y/z.md)";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", content)
        .file("x.md", "")
        .file("y/z.md", "")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "b/c.md"), content);
}

#[test]
fn rename_page_keeps_fragment_of_link() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[見出しへ](a.md#議題)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "[見出しへ](b.md#議題)");
}

#[test]
fn rename_page_keeps_link_title_attribute() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[a](a.md \"説明\")")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "[a](b.md \"説明\")");
}

#[test]
fn rename_page_rewrites_link_to_japanese_emoji_title() {
    let v = VaultBuilder::new()
        .file("会議.md", "")
        .file("other.md", "[会議メモ](会議.md)")
        .open();

    rename(&v, "会議.md", "定例📝");

    assert_eq!(text(&v, "other.md"), "[会議メモ](定例📝.md)");
}

// ================================================================ 書き換えないもの

#[test]
fn rename_page_does_not_rewrite_links_to_other_pages() {
    let content = "[x](x.md) [ab](ab.md) [ba](ba.md) [aa](a.md.md)";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("x.md", "")
        .file("ab.md", "")
        .file("ba.md", "")
        .file("a.md.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_does_not_rewrite_link_to_same_named_page_in_other_folder() {
    let content = "[別のa](x/a.md)";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("x/a.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_does_not_rewrite_relative_link_that_resolves_elsewhere() {
    // z/w.md から見た「a.md」は z/a.md を指す。ルートの a.md ではない。
    let content = "[z の a](a.md)";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("z/a.md", "")
        .file("z/w.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "z/w.md"), content);
}

#[test]
fn rename_page_does_not_rewrite_external_urls() {
    let content = "[外部](https://example.com/a.md) <https://example.com/a.md>";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_does_not_rewrite_wiki_style_links() {
    let content = "[[a]] と [[a.md]]";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_does_not_rewrite_plain_text_mentioning_file_name() {
    let content = "a.md というファイル名と (a.md) と [a] と a";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_does_not_rewrite_link_in_backtick_fenced_code_block() {
    let content = "```markdown\n[a](a.md)\n```\n";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_does_not_rewrite_link_in_tilde_fenced_code_block() {
    let content = "~~~\n[a](a.md)\n~~~\n";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_does_not_rewrite_link_in_long_fence_containing_shorter_fence() {
    let content = "````\n```\n[a](a.md)\n```\n````\n";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_does_not_rewrite_link_in_indented_code_block() {
    let content = "段落\n\n    [a](a.md)\n";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_does_not_rewrite_link_in_inline_code() {
    let content = "書き方は `[a](a.md)` と ``[a](a.md)`` です";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_rewrites_only_links_outside_code() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file(
            "other.md",
            "[本物](a.md)\n```\n[例](a.md)\n```\n`[例](a.md)` [本物](a.md)\n",
        )
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        text(&v, "other.md"),
        "[本物](b.md)\n```\n[例](a.md)\n```\n`[例](a.md)` [本物](b.md)\n"
    );
}

#[test]
fn rename_page_rewrites_link_after_unclosed_looking_backtick_text() {
    // 閉じていないバッククォートはコードではない(CommonMark)。
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "` だけ [a](a.md)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "` だけ [a](b.md)");
}

// ================================================================ 書き換えるページのそれ以外を守る

#[test]
fn rename_page_keeps_frontmatter_of_page_with_rewritten_link() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file(
            "other.md",
            "---\ntitle: \"参照: a\"\ntags: [x]\n---\n\n[a](a.md)\n",
        )
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        text(&v, "other.md"),
        "---\ntitle: \"参照: a\"\ntags: [x]\n---\n\n[a](b.md)\n"
    );
}

#[test]
fn rename_page_keeps_crlf_bom_and_missing_trailing_newline() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "\u{FEFF}1行目\r\n[a](a.md)  \r\n末尾")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "\u{FEFF}1行目\r\n[a](b.md)  \r\n末尾");
}

#[test]
fn rename_page_does_not_touch_pages_without_links_to_it() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("x.md", "[x](x.md) 無関係")
        .open();
    v.set_mtime("x.md", t(1_600_000_000));

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "x.md"), "[x](x.md) 無関係");
    assert_eq!(mtime(&v.root().join("x.md")), t(1_600_000_000));
}

#[test]
fn rename_page_skips_non_utf8_page_without_changing_a_byte() {
    // Shift_JIS の「あ」の後に、ASCII でリンクの形の文字列
    let mut bytes = vec![0x82, 0xA0];
    bytes.extend_from_slice(b" [a](a.md)");
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("sjis.md", &bytes)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(v.disk_bytes("sjis.md"), bytes);
}

#[test]
fn rename_page_leaves_no_temp_files() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("x.md", "[a](a.md)")
        .file("y/z.md", "[a](../a.md)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        v.all_entries(),
        ["b.md", "x.md", "y/", "y/z.md"].map(String::from).into()
    );
}

// ================================================================ エラー

fn assert_rename_err(v: &TestVault, path: &str, check: fn(&Error) -> bool) {
    let before = v.all_entries();
    let err = v.rename_page(path, "新しい名前").unwrap_err();
    assert!(check(&err), "{path:?}: {err:?}");
    assert_eq!(v.all_entries(), before, "{path:?}: vault が変わった");
}

#[test]
fn rename_page_returns_not_found_for_missing_page() {
    let v = VaultBuilder::new().file("a.md", "").open();
    assert_rename_err(&v, "none.md", |e| matches!(e, Error::NotFound(_)));
}

#[test]
fn rename_page_returns_not_found_for_page_deleted_outside_the_app() {
    let v = VaultBuilder::new().file("a.md", "").file("x.md", "").open();
    std::fs::remove_file(v.root().join("a.md")).unwrap();
    assert_rename_err(&v, "a.md", |e| matches!(e, Error::NotFound(_)));
}

#[test]
fn rename_page_rejects_non_canonical_paths() {
    let v = VaultBuilder::new()
        .file("a/b.md", "")
        .file("c.md", "")
        .outside_file("secret.md", "外")
        .open();
    for path in [
        "../secret.md",
        "a/../c.md",
        "./c.md",
        "a//b.md",
        "/c.md",
        "",
        "c\t.md",
    ] {
        assert_rename_err(&v, path, |e| matches!(e, Error::InvalidPath(_)));
    }
    assert!(v.outside("secret.md").exists());
}

#[test]
fn rename_page_rejects_hidden_file() {
    let v = VaultBuilder::new().file(".hidden.md", "").open();
    assert_rename_err(&v, ".hidden.md", |e| matches!(e, Error::NotAPage(_)));
}

#[test]
fn rename_page_rejects_non_markdown_file() {
    let v = VaultBuilder::new().file("notes.txt", "").open();
    assert_rename_err(&v, "notes.txt", |e| matches!(e, Error::NotAPage(_)));
}

#[test]
fn rename_page_rejects_folder_node() {
    let v = VaultBuilder::new().file("資料/b.md", "").open();
    assert_rename_err(&v, "資料", |e| matches!(e, Error::NotAPage(_)));
}

#[test]
fn rename_page_rejects_symlink() {
    let b = VaultBuilder::new().file("a.md", "");
    let target = b.root().join("a.md");
    let v = b.symlink("link.md", &target).open();
    assert_rename_err(&v, "link.md", |e| matches!(e, Error::NotAPage(_)));
}

// ================================================================ 仕様確認待ち

#[test]
#[ignore = "仕様確認待ち(質問33): リンクの表示文字列が旧タイトルと同じなら新タイトルに変えるか"]
fn rename_page_updates_link_text_equal_to_old_title() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[a](a.md)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "[b](b.md)");
}

#[test]
#[ignore = "仕様確認待ち(質問34): 名前を変えたページ自身の中にある、自分の子や自分へのリンクを書き換えるか"]
fn rename_page_rewrites_links_inside_renamed_page_to_its_children_and_itself() {
    let v = VaultBuilder::new()
        .file("a.md", "[子](a/c.md) [自分](a.md)")
        .file("a/c.md", "")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "b.md"), "[子](b/c.md) [自分](b.md)");
}
