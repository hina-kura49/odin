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

    assert_eq!(text(&v, "q/r/s.md"), "[b](../../p/b.md)");
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

    assert_eq!(text(&v, "other.md"), "[b](b.md \"説明\")");
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

    assert_eq!(text(&v, "other.md"), "` だけ [b](b.md)");
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
        "---\ntitle: \"参照: a\"\ntags: [x]\n---\n\n[b](b.md)\n"
    );
}

#[test]
fn rename_page_keeps_crlf_bom_and_missing_trailing_newline() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "\u{FEFF}1行目\r\n[a](a.md)  \r\n末尾")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "\u{FEFF}1行目\r\n[b](b.md)  \r\n末尾");
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

// ================================================================ 表示文字列(回答33)と自分自身の中のリンク(回答34)

#[test]
fn rename_page_updates_link_text_equal_to_old_title() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[a](a.md)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "[b](b.md)");
}

#[test]
fn rename_page_rewrites_links_inside_renamed_page_to_its_children_and_itself() {
    let v = VaultBuilder::new()
        .file("a.md", "[子](a/c.md) [自分](a.md)")
        .file("a/c.md", "")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "b.md"), "[子](b/c.md) [自分](b.md)");
}

#[test]
fn rename_page_updates_link_text_equal_to_old_title_compared_in_nfc() {
    let v = VaultBuilder::new()
        .file("がっこう.md", "")
        .file("other.md", format!("[{}](がっこう.md)", nfd("がっこう")))
        .open();

    rename(&v, "がっこう.md", "学校");

    assert_eq!(text(&v, "other.md"), "[学校](学校.md)");
}

#[test]
fn rename_page_keeps_link_text_that_differs_even_slightly_from_old_title() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[a ](a.md) [A](a.md) [a.md](a.md) [aa](a.md)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        text(&v, "other.md"),
        "[a ](b.md) [A](b.md) [a.md](b.md) [aa](b.md)"
    );
}

#[test]
fn rename_page_keeps_old_title_text_of_links_to_other_pages() {
    let content = "[a](x.md)";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("x.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_rewrites_only_changed_links_in_child_pages() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file(
            "a/c.md",
            "[親](../a.md) [兄弟](d.md) [自分](../a/c.md) [外](../x.md)",
        )
        .file("a/d.md", "")
        .file("x.md", "")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        text(&v, "b/c.md"),
        "[親](../b.md) [兄弟](d.md) [自分](../b/c.md) [外](../x.md)"
    );
}

// ================================================================ 新しいタイトルの処理と重なり(回答35)

#[test]
fn rename_page_processes_new_title_like_create_page() {
    let v = VaultBuilder::new().file("a.md", "").open();

    let meta = rename(&v, "a.md", &format!("  {}/x:y  ", nfd(".が")));

    assert_eq!(meta.title, nfc("-が-x-y"));
    assert_eq!(meta.path, nfc("-が-x-y.md"));
}

#[test]
fn rename_page_uses_untitled_for_empty_new_title() {
    let v = VaultBuilder::new().file("a.md", "").open();

    assert_eq!(rename(&v, "a.md", " \u{3000} ").title, "無題");
}

#[test]
fn rename_page_truncates_new_title_over_200_bytes() {
    let v = VaultBuilder::new().file("a.md", "").open();

    assert_eq!(rename(&v, "a.md", &"あ".repeat(101)).title, "あ".repeat(66));
}

fn assert_name_occupied(v: &TestVault, path: &str, new_title: &str) {
    let before = all_entries(&v.root());
    let bytes: Vec<_> = before
        .iter()
        .filter(|e| !e.ends_with('/'))
        .map(|e| (e.clone(), v.disk_bytes(e)))
        .collect();

    let err = v.rename_page(path, new_title).unwrap_err();

    assert!(matches!(err, Error::NameOccupied(_)), "{err:?}");
    assert_eq!(v.all_entries(), before);
    for (rel, b) in bytes {
        assert_eq!(v.disk_bytes(&rel), b, "{rel} が変わった");
    }
}

#[test]
fn rename_page_returns_name_occupied_when_page_with_new_name_exists() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("b.md", "既存")
        .file("x.md", "[a](a.md)")
        .open();
    assert_name_occupied(&v, "a.md", "b");
}

#[test]
fn rename_page_does_not_number_colliding_title() {
    let v = VaultBuilder::new().file("a.md", "").file("b.md", "").open();

    let err = v.rename_page("a.md", "b").unwrap_err();

    assert!(matches!(err, Error::NameOccupied(_)), "{err:?}");
    assert!(!v.exists("b 2.md"));
}

#[test]
fn rename_page_returns_name_occupied_for_existing_name_differing_in_case_or_normalization() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("B.md", "")
        .file(&nfd("が.md"), "")
        .open();
    assert_name_occupied(&v, "a.md", "b");
    assert_name_occupied(&v, "a.md", &nfc("が"));
}

#[test]
fn rename_page_returns_name_occupied_for_existing_page_with_uppercase_extension() {
    let v = VaultBuilder::new().file("a.md", "").file("b.MD", "").open();
    assert_name_occupied(&v, "a.md", "b");
}

#[test]
fn rename_page_checks_collision_only_in_the_same_folder() {
    let v = VaultBuilder::new()
        .file("p.md", "")
        .file("p/a.md", "")
        .file("b.md", "")
        .open();

    assert_eq!(rename(&v, "p/a.md", "b").path, "p/b.md");
}

#[test]
fn page_without_children_renamed_to_folder_name_adopts_that_folder() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("b/x.md", "")
        .open();

    let meta = rename(&v, "a.md", "b");

    assert_eq!(meta.path, "b.md");
    let tree = v.list_tree().unwrap();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Page);
    assert_eq!(all_paths(&tree[0].children), ["b/x.md".to_string()].into());
}

#[test]
fn rename_page_with_children_returns_name_occupied_when_folder_with_new_name_exists() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", "")
        .file("b/x.md", "")
        .open();
    assert_name_occupied(&v, "a.md", "b");
}

#[test]
fn rename_page_with_children_returns_name_occupied_when_file_blocks_new_folder() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", "")
        .file("b", "拡張子なし")
        .open();
    assert_name_occupied(&v, "a.md", "b");
}

// ================================================================ リンクの書き方の違い(回答36)

#[test]
fn rename_page_rewrites_link_differing_only_in_case() {
    let v = VaultBuilder::new()
        .file("Note.md", "")
        .file("other.md", "[x](NOTE.md) [y](note/c.md)")
        .file("Note/c.md", "")
        .open();

    rename(&v, "Note.md", "メモ");

    assert_eq!(text(&v, "other.md"), "[x](メモ.md) [y](メモ/c.md)");
}

#[test]
fn rename_page_rewrites_link_differing_only_in_normalization() {
    let v = VaultBuilder::new()
        .file(&nfc("がっこう.md"), "")
        .file("other.md", format!("[x]({})", nfd("がっこう.md")))
        .open();

    rename(&v, &nfc("がっこう.md"), "学校");

    assert_eq!(text(&v, "other.md"), "[x](学校.md)");
}

#[test]
fn rename_page_rewrites_percent_encoded_link() {
    let v = VaultBuilder::new()
        .file("会議.md", "")
        .file("other.md", "[x](%E4%BC%9A%E8%AD%B0.md)")
        .open();

    rename(&v, "会議.md", "定例");

    assert_eq!(text(&v, "other.md"), "[x](定例.md)");
}

#[test]
fn rename_page_keeps_leading_dot_slash() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[x](./a.md) [y](./a/c.md)")
        .file("a/c.md", "")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "[x](./b.md) [y](./b/c.md)");
}

#[test]
fn rename_page_keeps_angle_brackets() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[x](<a.md>) [y](<./a.md#見出し> \"説明\")")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        text(&v, "other.md"),
        "[x](<b.md>) [y](<./b.md#見出し> \"説明\")"
    );
}

#[test]
fn rename_page_keeps_original_spelling_of_unchanged_parts() {
    // フォルダ部分(%20)と子ページ部分(%E5%AD%90 =「子」)は変わらないので、元の書き方のまま。
    let v = VaultBuilder::new()
        .file("資 料/a.md", "")
        .file("資 料/a/子.md", "")
        .file("other.md", "[x](資%20料/a.md) [y](資%20料/a/%E5%AD%90.md)")
        .open();

    rename(&v, "資 料/a.md", "b");

    assert_eq!(
        text(&v, "other.md"),
        "[x](資%20料/b.md) [y](資%20料/b/%E5%AD%90.md)"
    );
}

// ================================================================ 新しい名前の書き方(回答37)

#[test]
fn rename_page_percent_encodes_space_in_new_name() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[x](a.md)")
        .open();

    rename(&v, "a.md", "定例 会議");

    assert_eq!(text(&v, "other.md"), "[x](定例%20会議.md)");
}

#[test]
fn rename_page_percent_encodes_reserved_characters_in_new_name() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[x](a.md)")
        .open();

    rename(&v, "a.md", "b(1)<2>#3%4?5");

    assert_eq!(text(&v, "other.md"), "[x](b%281%29%3C2%3E%233%254%3F5.md)");
}

#[test]
fn rename_page_writes_japanese_and_emoji_in_new_name_as_is() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[x](a.md)")
        .open();

    rename(&v, "a.md", "会議📝ＡＢＣ");

    assert_eq!(text(&v, "other.md"), "[x](会議📝ＡＢＣ.md)");
}

#[test]
fn rename_page_encodes_new_name_inside_angle_brackets_too() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[x](<a.md>)")
        .open();

    rename(&v, "a.md", "b c");

    assert_eq!(text(&v, "other.md"), "[x](<b%20c.md>)");
}

#[test]
fn rename_page_does_not_wrap_new_name_in_angle_brackets() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "[x](a/c.md)")
        .file("a/c.md", "")
        .open();

    rename(&v, "a.md", "b c");

    assert_eq!(text(&v, "other.md"), "[x](b%20c/c.md)");
}

// ================================================================ 画像・参照形式・HTML(回答38)

#[test]
fn rename_page_rewrites_image_in_child_folder_from_other_page() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/pic.png", "png")
        .file("other.md", "![図](a/pic.png)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "![図](b/pic.png)");
}

#[test]
fn rename_page_rewrites_image_in_child_folder_from_renamed_page_itself() {
    let v = VaultBuilder::new()
        .file("a.md", "![図](a/pic.png)")
        .file("a/pic.png", "png")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "b.md"), "![図](b/pic.png)");
}

#[test]
fn rename_page_keeps_image_link_inside_child_folder() {
    let content = "![図](pic.png)";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", content)
        .file("a/pic.png", "png")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "b/c.md"), content);
}

#[test]
fn rename_page_rewrites_link_to_non_markdown_file_in_child_folder() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/資料.pdf", "%PDF")
        .file("other.md", "[資料](a/資料.pdf)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), "[資料](b/資料.pdf)");
}

#[test]
fn rename_page_rewrites_reference_definitions() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file(
            "other.md",
            "[本文][r1] と [図][r2]\n\n[r1]: a.md\n[r2]: <./a/pic.png> \"説明\"\n",
        )
        .file("a/pic.png", "png")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        text(&v, "other.md"),
        "[本文][r1] と [図][r2]\n\n[r1]: b.md\n[r2]: <./b/pic.png> \"説明\"\n"
    );
}

#[test]
fn rename_page_does_not_rewrite_html_attributes() {
    let content = "<a href=\"a.md\">a</a> <img src=\"a/pic.png\">\n";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/pic.png", "png")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

#[test]
fn rename_page_does_not_rewrite_bare_angle_bracket_link() {
    let content = "<a.md> と <./a.md>";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", content)
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "other.md"), content);
}

// ================================================================ 隠しファイル・読み取り専用(回答39)

#[test]
fn rename_page_does_not_rewrite_links_in_hidden_files() {
    let content = "[a](a.md)";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file(".hidden.md", content)
        .file(".obsidian/x.md", "[a](../a.md)")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, ".hidden.md"), content);
    assert_eq!(text(&v, ".obsidian/x.md"), "[a](../a.md)");
}

#[test]
fn rename_page_returns_read_only_when_a_page_needing_rewrite_is_read_only() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", "")
        .file("x.md", "[a](a.md)")
        .file("y.md", "[a](a.md)")
        .open();
    v.set_mode("y.md", 0o444);

    let err = v.rename_page("a.md", "b").unwrap_err();

    assert!(matches!(err, Error::ReadOnly(_)), "{err:?}");
}

#[test]
fn read_only_page_needing_rewrite_leaves_vault_unchanged() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/c.md", "")
        .file("x.md", "[a](a.md)")
        .file("y.md", "[a](a.md)")
        .open();
    v.set_mode("y.md", 0o444);
    let before = snapshot(&v);

    let _ = v.rename_page("a.md", "b");

    assert_eq!(snapshot(&v), before);
}

#[test]
fn read_only_page_without_links_to_target_does_not_block_rename() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("x.md", "[a](a.md)")
        .file("ro.md", "無関係")
        .open();
    v.set_mode("ro.md", 0o444);

    rename(&v, "a.md", "b");

    assert_eq!(text(&v, "x.md"), "[b](b.md)");
}

// ================================================================ フロントマターと HTML コメント(回答40)

#[test]
fn rename_page_does_not_rewrite_link_like_text_in_frontmatter() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("other.md", "---\nrelated: \"[a](a.md)\"\n---\n[a](a.md)\n")
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        text(&v, "other.md"),
        "---\nrelated: \"[a](a.md)\"\n---\n[b](b.md)\n"
    );
}

#[test]
fn rename_page_does_not_rewrite_link_in_html_comment() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file(
            "other.md",
            "<!-- [a](a.md)\n![](a/pic.png) -->\n[a](a.md)\n",
        )
        .open();

    rename(&v, "a.md", "b");

    assert_eq!(
        text(&v, "other.md"),
        "<!-- [a](a.md)\n![](a/pic.png) -->\n[b](b.md)\n"
    );
}

// ================================================================ 失敗したら元通り(回答41)

/// フォルダの権限を一時的に変え、テストが終わったら(失敗しても)元に戻す。
struct ModeGuard(std::path::PathBuf);

impl Drop for ModeGuard {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

fn lock_dir(path: std::path::PathBuf) -> ModeGuard {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o555)).unwrap();
    ModeGuard(path)
}

/// すべての名前と中身(フォルダは空として記録する)。
fn snapshot(v: &TestVault) -> std::collections::BTreeMap<String, Vec<u8>> {
    v.all_entries()
        .into_iter()
        .map(|rel| {
            let b = if rel.ends_with('/') {
                Vec::new()
            } else {
                v.disk_bytes(&rel)
            };
            (rel, b)
        })
        .collect()
}

#[test]
fn rename_page_fails_when_page_with_link_is_in_unwritable_folder() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("x.md", "[a](a.md)")
        .file("locked/y.md", "[a](../a.md)")
        .open();
    let _guard = lock_dir(v.root().join("locked"));

    assert!(v.rename_page("a.md", "b").is_err());
}

#[test]
fn failed_rename_leaves_vault_exactly_as_before() {
    let v = VaultBuilder::new()
        .file("a.md", "[自分](a.md)")
        .file("a/c.md", "[親](../a.md)")
        .file("x.md", "[a](a.md)")
        .file("locked/y.md", "[a](../a.md)")
        .file("z.md", "[a](a.md)")
        .open();
    let before = snapshot(&v);
    let guard = lock_dir(v.root().join("locked"));

    let _ = v.rename_page("a.md", "b");

    drop(guard);
    assert_eq!(snapshot(&v), before);
}

#[test]
fn failed_rename_of_page_in_unwritable_folder_leaves_links_untouched() {
    let v = VaultBuilder::new()
        .file("p/a.md", "")
        .file("x.md", "[a](p/a.md)")
        .open();
    let before = snapshot(&v);
    let guard = lock_dir(v.root().join("p"));

    let result = v.rename_page("p/a.md", "b");

    drop(guard);
    assert!(result.is_err());
    assert_eq!(snapshot(&v), before);
}

// ================================================================ 大文字小文字だけ・同じ名前(回答42・43)

#[test]
fn rename_page_changes_letter_case_only() {
    let v = VaultBuilder::new()
        .file("note.md", "")
        .file("note/c.md", "")
        .file("other.md", "[x](note.md) [y](note/c.md)")
        .open();

    let meta = rename(&v, "note.md", "Note");

    assert_eq!(meta.path, "Note.md");
    assert_eq!(
        names_on_disk(&v.root()),
        ["Note", "Note.md", "other.md"].map(String::from).into()
    );
    assert_eq!(text(&v, "other.md"), "[x](Note.md) [y](Note/c.md)");
}

#[test]
fn rename_page_to_exactly_same_name_changes_nothing() {
    let v = VaultBuilder::new()
        .file("a.md", "[自分](a.md)")
        .file("a/c.md", "[親](../a.md)")
        .file("x.md", "[a](a.md)")
        .open();
    for rel in ["a.md", "a/c.md", "x.md"] {
        v.set_mtime(rel, t(1_600_000_000));
    }
    let before = snapshot(&v);

    let meta = rename(&v, "a.md", " a ");

    assert_eq!(meta.path, "a.md");
    assert_eq!(snapshot(&v), before);
    for rel in ["a.md", "a/c.md", "x.md"] {
        assert_eq!(mtime(&v.root().join(rel)), t(1_600_000_000), "{rel}");
    }
}

#[test]
fn rename_page_writes_lowercase_md_extension() {
    let v = VaultBuilder::new().file("a.MD", "").open();

    rename(&v, "a.MD", "b");

    assert_eq!(names_on_disk(&v.root()), ["b.md".to_string()].into());
}

#[test]
fn rename_page_to_same_name_keeps_uppercase_extension() {
    let v = VaultBuilder::new().file("a.MD", "").open();

    let meta = rename(&v, "a.md", "a");

    assert_eq!(meta.path, "a.MD");
    assert_eq!(names_on_disk(&v.root()), ["a.MD".to_string()].into());
}

// ================================================================ UTF-8 でないページ(回答44)

#[test]
fn rename_page_renames_non_utf8_page_without_touching_its_content() {
    let v = VaultBuilder::new()
        .file("sjis.md", [0x82, 0xA0])
        .file("other.md", "[x](sjis.md)")
        .open();

    rename(&v, "sjis.md", "b");

    assert_eq!(v.disk_bytes("b.md"), [0x82, 0xA0]);
    assert_eq!(text(&v, "other.md"), "[x](b.md)");
}
