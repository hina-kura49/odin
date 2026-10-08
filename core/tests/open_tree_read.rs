//! 段階1: open / list_tree / read_page

mod common;

use std::time::{Duration, SystemTime};

use common::*;
use odin_core::{Error, NodeKind, Vault};

fn t(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

// ================================================================ open

#[test]
fn open_succeeds_on_existing_empty_directory() {
    let b = VaultBuilder::new();
    assert!(Vault::open(&b.root(), &b.index_dir()).is_ok());
}

#[test]
fn open_fails_when_root_does_not_exist() {
    let b = VaultBuilder::new();
    assert!(Vault::open(&b.root().join("missing"), &b.index_dir()).is_err());
}

#[test]
fn open_fails_when_root_is_a_file() {
    let b = VaultBuilder::new().file("a.md", "x");
    assert!(Vault::open(&b.root().join("a.md"), &b.index_dir()).is_err());
}

#[test]
fn open_does_not_modify_existing_pages() {
    let b = VaultBuilder::new().file("a.md", "---\ntitle: x\n---\r\n本文");
    let path = b.root().join("a.md");
    set_mtime(&path, t(1_600_000_000));

    let v = b.open();

    assert_eq!(
        v.disk_bytes("a.md"),
        "---\ntitle: x\n---\r\n本文".as_bytes()
    );
    assert_eq!(mtime(&path), t(1_600_000_000));
}

#[test]
fn open_adds_no_non_markdown_entries_to_vault() {
    let b = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .file("img.png", "x");
    let before = non_md_entries(&b.root());

    let v = b.open();

    assert_eq!(v.non_md_entries(), before);
}

#[test]
fn vault_is_send_and_sync() {
    // Tauri の管理状態(State)に載せるために必要。
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Vault>();
}

// ================================================================ list_tree: 中身

#[test]
fn list_tree_of_empty_vault_is_empty() {
    let v = VaultBuilder::new().open();
    assert!(v.list_tree().unwrap().is_empty());
}

#[test]
fn list_tree_shows_md_file_as_page_titled_by_file_name_without_extension() {
    let v = VaultBuilder::new().file("買い物.md", "").open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Page);
    assert_eq!(tree[0].path, "買い物.md");
    assert_eq!(tree[0].title, "買い物");
    assert!(tree[0].children.is_empty());
}

#[test]
fn list_tree_removes_only_the_md_extension_from_title() {
    let v = VaultBuilder::new().file("v1.2 リリース.md", "").open();

    let tree = v.list_tree().unwrap();

    assert_eq!(
        find(&tree, "v1.2 リリース.md").unwrap().title,
        "v1.2 リリース"
    );
}

#[test]
fn list_tree_treats_uppercase_md_extension_as_page() {
    let v = VaultBuilder::new().file("大文字.MD", "").open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Page);
    assert_eq!(tree[0].path, "大文字.MD");
    assert_eq!(tree[0].title, "大文字");
}

#[test]
fn list_tree_excludes_markdown_extension_other_than_md() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("b.markdown", "")
        .file("c.mdx", "")
        .open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["a.md".to_string()].into()
    );
}

#[test]
fn list_tree_shows_all_top_level_pages() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("b.md", "")
        .file("c.md", "")
        .open();

    let paths = all_paths(&v.list_tree().unwrap());

    assert_eq!(paths, ["a.md", "b.md", "c.md"].map(String::from).into());
}

#[test]
fn list_tree_puts_pages_in_same_named_folder_under_that_page() {
    let v = VaultBuilder::new()
        .file("仕事.md", "")
        .file("仕事/会議.md", "")
        .file("仕事/日報.md", "")
        .open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Page);
    assert_eq!(tree[0].path, "仕事.md");
    assert_eq!(
        all_paths(&tree[0].children),
        ["仕事/会議.md", "仕事/日報.md"].map(String::from).into()
    );
}

#[test]
fn list_tree_nests_grandchildren_with_slash_separated_paths() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .file("a/b/c.md", "")
        .open();

    let tree = v.list_tree().unwrap();

    let a = find(&tree, "a.md").unwrap();
    let b = find(&a.children, "a/b.md").unwrap();
    assert_eq!(b.title, "b");
    assert_eq!(b.children.len(), 1);
    assert_eq!(b.children[0].path, "a/b/c.md");
    assert_eq!(b.children[0].title, "c");
}

#[test]
fn list_tree_shows_folder_without_same_named_page_as_folder_node() {
    let v = VaultBuilder::new().file("資料/b.md", "").open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Folder);
    assert_eq!(tree[0].path, "資料");
    assert_eq!(tree[0].title, "資料");
    assert_eq!(
        all_paths(&tree[0].children),
        ["資料/b.md".to_string()].into()
    );
}

#[test]
fn list_tree_merges_page_and_same_named_folder_into_one_page_node() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Page);
    assert_eq!(tree[0].path, "a.md");
}

#[test]
fn list_tree_nests_folder_node_inside_page() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/x/y.md", "")
        .open();

    let tree = v.list_tree().unwrap();

    let x = find(&tree, "a/x").unwrap();
    assert_eq!(x.kind, NodeKind::Folder);
    assert_eq!(x.title, "x");
    assert_eq!(all_paths(&x.children), ["a/x/y.md".to_string()].into());
}

#[test]
fn list_tree_excludes_hidden_files() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file(".hidden.md", "")
        .file("a/.secret.md", "")
        .open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["a.md".to_string()].into()
    );
}

#[test]
fn list_tree_excludes_hidden_folders_and_their_pages() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file(".obsidian/workspace.md", "")
        .file(".trash/old.md", "")
        .open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["a.md".to_string()].into()
    );
}

#[test]
fn list_tree_excludes_non_markdown_files() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("photo.png", [0x89, b'P', b'N', b'G'])
        .file("notes.txt", "text")
        .file("a/attachment.pdf", "%PDF")
        .file("markdown", "拡張子なし")
        .open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["a.md".to_string()].into()
    );
}

#[test]
fn list_tree_excludes_symlink_to_page_inside_vault() {
    let b = VaultBuilder::new().file("a.md", "");
    let target = b.root().join("a.md");
    let v = b.symlink("link.md", &target).open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["a.md".to_string()].into()
    );
}

#[test]
fn list_tree_excludes_symlink_to_page_outside_vault() {
    let b = VaultBuilder::new()
        .file("a.md", "")
        .outside_file("secret.md", "外");
    let target = b.outside("secret.md");
    let v = b.symlink("link.md", &target).open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["a.md".to_string()].into()
    );
}

#[test]
fn list_tree_excludes_symlinked_folder() {
    let b = VaultBuilder::new().file("a.md", "").file("real/b.md", "");
    let target = b.root().join("real");
    let v = b.symlink("linked", &target).open();

    let paths = all_paths(&v.list_tree().unwrap());

    assert!(!paths.iter().any(|p| p.starts_with("linked")), "{paths:?}");
}

#[test]
fn list_tree_shows_non_utf8_page() {
    // Shift_JIS の「あ」
    let v = VaultBuilder::new().file("sjis.md", [0x82, 0xA0]).open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["sjis.md".to_string()].into()
    );
}

#[test]
fn list_tree_handles_japanese_emoji_and_mixed_width_names() {
    let name = "メモ📝 ＡＢＣabc １２３123 ｶﾀｶﾅ";
    let v = VaultBuilder::new().file(&format!("{name}.md"), "").open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].title, name);
}

#[test]
fn list_tree_returns_nfd_page_name_in_nfc() {
    // 「が」を「か」+ 結合用濁点(U+3099)で書いた名前
    let v = VaultBuilder::new()
        .file(&format!("{}.md", nfd("がっこう")), "")
        .open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree[0].path, nfc("がっこう.md"));
    assert_eq!(tree[0].title, nfc("がっこう"));
}

#[test]
fn list_tree_returns_nfd_folder_and_child_paths_in_nfc() {
    let v = VaultBuilder::new()
        .file(&format!("{}/{}.md", nfd("ぶんしょ"), nfd("ばんごう")), "")
        .open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree[0].path, nfc("ぶんしょ"));
    assert_eq!(tree[0].children[0].path, nfc("ぶんしょ/ばんごう.md"));
}

#[test]
fn list_tree_keeps_letter_case_of_file_name() {
    let v = VaultBuilder::new().file("README.md", "").open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree[0].title, "README");
    assert_eq!(tree[0].path, "README.md");
}

#[test]
fn list_tree_includes_empty_files() {
    let v = VaultBuilder::new().file("空.md", "").open();
    assert!(find(&v.list_tree().unwrap(), "空.md").is_some());
}

#[test]
fn list_tree_reflects_files_added_outside_the_app_after_rescan() {
    let v = VaultBuilder::new().file("a.md", "").open();
    v.write_outside_app("b.md", "");
    v.rescan().unwrap();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["a.md", "b.md"].map(String::from).into()
    );
}

#[test]
fn list_tree_adds_no_non_markdown_entries_to_vault() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .open();
    let before = v.non_md_entries();

    v.list_tree().unwrap();

    assert_eq!(v.non_md_entries(), before);
}

#[test]
fn every_path_from_list_tree_reaches_the_same_page_via_read_page() {
    let v = VaultBuilder::new()
        .file(&format!("{}.md", nfd("がっこう")), "1")
        .file(&format!("{}/{}.md", nfd("がっこう"), nfd("ぱん")), "2")
        .file("大文字.MD", "3")
        .file("フォルダ/子.md", "4")
        .open();

    let mut contents = Vec::new();
    fn walk(v: &Vault, nodes: &[odin_core::TreeNode], out: &mut Vec<String>) {
        for n in nodes {
            if n.kind == NodeKind::Page {
                out.push(v.read_page(&n.path).unwrap().0);
            }
            walk(v, &n.children, out);
        }
    }
    walk(&v, &v.list_tree().unwrap(), &mut contents);
    contents.sort();

    assert_eq!(contents, ["1", "2", "3", "4"]);
}

// ================================================================ list_tree: 並び順

#[test]
fn list_tree_sorts_siblings_by_name() {
    let v = VaultBuilder::new()
        .file("c.md", "")
        .file("a.md", "")
        .file("b.md", "")
        .open();

    assert_eq!(titles(&v.list_tree().unwrap()), ["a", "b", "c"]);
}

#[test]
fn list_tree_sorts_ignoring_letter_case() {
    let v = VaultBuilder::new()
        .file("b.md", "")
        .file("C.md", "")
        .file("A.md", "")
        .open();

    assert_eq!(titles(&v.list_tree().unwrap()), ["A", "b", "C"]);
}

#[test]
fn list_tree_sorts_numbers_by_numeric_value() {
    let v = VaultBuilder::new()
        .file("メモ10.md", "")
        .file("メモ2.md", "")
        .file("メモ1.md", "")
        .open();

    assert_eq!(
        titles(&v.list_tree().unwrap()),
        ["メモ1", "メモ2", "メモ10"]
    );
}

#[test]
fn list_tree_sorts_numbers_numerically_regardless_of_letter_case() {
    let v = VaultBuilder::new()
        .file("Page10.md", "")
        .file("page2.md", "")
        .file("PAGE1.md", "")
        .open();

    assert_eq!(
        titles(&v.list_tree().unwrap()),
        ["PAGE1", "page2", "Page10"]
    );
}

#[test]
fn list_tree_sorts_after_normalizing_to_nfc() {
    // NFC で比べると「かカ」(か U+304B…) < 「が」(U+304C)。
    // NFD のまま比べると「が」= か + U+3099 となり、カ(U+30AB)より前に来てしまう。
    let v = VaultBuilder::new()
        .file(&format!("{}.md", nfd("が")), "")
        .file("かカ.md", "")
        .open();

    assert_eq!(titles(&v.list_tree().unwrap()), ["かカ", "が"]);
}

#[test]
fn list_tree_sorts_pages_and_folders_together() {
    let v = VaultBuilder::new()
        .file("c.md", "")
        .file("b/x.md", "")
        .file("a.md", "")
        .file("d/y.md", "")
        .open();

    let tree = v.list_tree().unwrap();

    assert_eq!(titles(&tree), ["a", "b", "c", "d"]);
    assert_eq!(tree[1].kind, NodeKind::Folder);
}

#[test]
fn list_tree_sorts_children_too() {
    let v = VaultBuilder::new()
        .file("p.md", "")
        .file("p/項目10.md", "")
        .file("p/項目2.md", "")
        .file("p/Z/x.md", "")
        .file("p/a.md", "")
        .open();

    let tree = v.list_tree().unwrap();

    assert_eq!(titles(&tree[0].children), ["a", "Z", "項目2", "項目10"]);
}

// ================================================================ read_page: 内容

#[test]
fn read_page_returns_file_content() {
    let v = VaultBuilder::new()
        .file("a.md", "# 見出し\n\n本文です。\n")
        .open();

    let (content, _) = v.read_page("a.md").unwrap();

    assert_eq!(content, "# 見出し\n\n本文です。\n");
}

#[test]
fn read_page_reads_nested_page_by_slash_separated_path() {
    let v = VaultBuilder::new()
        .file("a.md", "親")
        .file("a/b/c.md", "孫")
        .open();

    assert_eq!(v.read_page("a/b/c.md").unwrap().0, "孫");
}

#[test]
fn read_page_reads_page_with_uppercase_md_extension() {
    let v = VaultBuilder::new().file("a.MD", "中身").open();
    assert_eq!(v.read_page("a.MD").unwrap().0, "中身");
}

#[test]
fn read_page_returns_empty_string_for_empty_file() {
    let v = VaultBuilder::new().file("a.md", "").open();
    assert_eq!(v.read_page("a.md").unwrap().0, "");
}

#[test]
fn read_page_preserves_crlf() {
    let v = VaultBuilder::new()
        .file("a.md", "1行目\r\n2行目\r\n")
        .open();
    assert_eq!(v.read_page("a.md").unwrap().0, "1行目\r\n2行目\r\n");
}

#[test]
fn read_page_preserves_missing_trailing_newline_and_trailing_spaces() {
    let v = VaultBuilder::new()
        .file("a.md", "行末に空白  \n最後の行  ")
        .open();
    assert_eq!(v.read_page("a.md").unwrap().0, "行末に空白  \n最後の行  ");
}

#[test]
fn read_page_returns_frontmatter_as_part_of_content() {
    let text = "---\ntitle: \"予定\"\ntags: [a, b]\n---\n\n# 本文\n";
    let v = VaultBuilder::new().file("a.md", text).open();

    assert_eq!(v.read_page("a.md").unwrap().0, text);
}

#[test]
fn read_page_handles_japanese_emoji_and_mixed_width_content() {
    let text = "絵文字👨‍👩‍👧と国旗🇯🇵、全角ＡＢＣと半角ABC、ｶﾀｶﾅ\u{3000}全角空白";
    let v = VaultBuilder::new().file("a.md", text).open();

    assert_eq!(v.read_page("a.md").unwrap().0, text);
}

#[test]
fn read_page_returns_multi_megabyte_file_intact() {
    let text = "あいうえおabcde🍣\n".repeat(200_000); // 約 4.8MB
    let v = VaultBuilder::new().file("big.md", &text).open();

    let (content, _) = v.read_page("big.md").unwrap();

    assert_eq!(content.len(), text.len());
    assert!(content == text);
}

#[test]
fn read_page_finds_nfd_named_file_by_nfc_path() {
    let v = VaultBuilder::new()
        .file(&format!("{}.md", nfd("がっこう")), "中身")
        .open();

    assert_eq!(v.read_page(&nfc("がっこう.md")).unwrap().0, "中身");
}

#[test]
fn read_page_finds_nfc_named_file_by_nfd_path() {
    let v = VaultBuilder::new()
        .file(&format!("{}.md", nfc("がっこう")), "中身")
        .open();

    assert_eq!(v.read_page(&nfd("がっこう.md")).unwrap().0, "中身");
}

#[test]
fn read_page_finds_page_in_nfd_named_folder_by_nfc_path() {
    let v = VaultBuilder::new()
        .file(
            &format!("{}/{}.md", nfd("ぶんしょ"), nfd("ばんごう")),
            "中身",
        )
        .open();

    assert_eq!(v.read_page(&nfc("ぶんしょ/ばんごう.md")).unwrap().0, "中身");
}

#[test]
fn read_page_adds_no_non_markdown_entries_to_vault() {
    let v = VaultBuilder::new().file("a.md", "x").open();
    let before = v.non_md_entries();

    v.read_page("a.md").unwrap();

    assert_eq!(v.non_md_entries(), before);
}

// ================================================================ read_page: version

#[test]
fn read_page_returns_same_version_while_file_is_unchanged() {
    let v = VaultBuilder::new().file("a.md", "x").open();

    let (_, first) = v.read_page("a.md").unwrap();
    let (_, second) = v.read_page("a.md").unwrap();

    assert_eq!(first, second);
}

#[test]
fn read_page_returns_same_version_when_only_mtime_changes() {
    let v = VaultBuilder::new().file("a.md", "x").open();
    v.set_mtime("a.md", t(1_600_000_000));
    let (_, before) = v.read_page("a.md").unwrap();

    v.set_mtime("a.md", t(1_700_000_000));
    let (_, after) = v.read_page("a.md").unwrap();

    assert_eq!(before, after);
}

#[test]
fn read_page_returns_same_version_when_rewritten_with_same_content() {
    let v = VaultBuilder::new().file("a.md", "同じ内容").open();
    let (_, before) = v.read_page("a.md").unwrap();

    v.write_outside_app("a.md", "同じ内容");
    let (_, after) = v.read_page("a.md").unwrap();

    assert_eq!(before, after);
}

#[test]
fn read_page_returns_different_version_when_content_changes_with_same_mtime() {
    // 同じ1秒内(ここでは mtime を完全に同じに戻す)の書き換えでも version が変わること。
    let v = VaultBuilder::new().file("a.md", "内容A").open();
    v.set_mtime("a.md", t(1_600_000_000));
    let (_, before) = v.read_page("a.md").unwrap();

    v.write_outside_app("a.md", "内容B"); // 同じバイト長
    v.set_mtime("a.md", t(1_600_000_000));
    let (_, after) = v.read_page("a.md").unwrap();

    assert_ne!(before, after);
}

#[test]
fn read_page_returns_different_version_when_only_line_endings_change() {
    let v = VaultBuilder::new().file("a.md", "a\nb\n").open();
    let (_, before) = v.read_page("a.md").unwrap();

    v.write_outside_app("a.md", "a\r\nb\r\n");
    let (_, after) = v.read_page("a.md").unwrap();

    assert_ne!(before, after);
}

// ================================================================ read_page: エラー

#[test]
fn read_page_returns_not_found_for_missing_page() {
    let v = VaultBuilder::new().file("a.md", "").open();

    let err = v.read_page("none.md").unwrap_err();

    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

#[test]
fn read_page_returns_not_found_for_page_deleted_outside_the_app() {
    let v = VaultBuilder::new().file("a.md", "").open();
    std::fs::remove_file(v.root().join("a.md")).unwrap();

    let err = v.read_page("a.md").unwrap_err();

    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

#[test]
fn read_page_returns_not_utf8_error_for_non_utf8_page() {
    let v = VaultBuilder::new().file("sjis.md", [0x82, 0xA0]).open();

    let err = v.read_page("sjis.md").unwrap_err();

    assert!(matches!(err, Error::NotUtf8(_)), "{err:?}");
}

#[test]
fn read_page_rejects_hidden_file_as_not_a_page() {
    let v = VaultBuilder::new().file(".hidden.md", "x").open();

    let err = v.read_page(".hidden.md").unwrap_err();

    assert!(matches!(err, Error::NotAPage(_)), "{err:?}");
}

#[test]
fn read_page_rejects_page_inside_hidden_folder_as_not_a_page() {
    let v = VaultBuilder::new().file(".obsidian/a.md", "x").open();

    let err = v.read_page(".obsidian/a.md").unwrap_err();

    assert!(matches!(err, Error::NotAPage(_)), "{err:?}");
}

#[test]
fn read_page_rejects_non_markdown_file_as_not_a_page() {
    let v = VaultBuilder::new().file("notes.txt", "x").open();

    let err = v.read_page("notes.txt").unwrap_err();

    assert!(matches!(err, Error::NotAPage(_)), "{err:?}");
}

#[test]
fn read_page_rejects_dot_markdown_file_as_not_a_page() {
    let v = VaultBuilder::new().file("a.markdown", "x").open();

    let err = v.read_page("a.markdown").unwrap_err();

    assert!(matches!(err, Error::NotAPage(_)), "{err:?}");
}

#[test]
fn read_page_rejects_folder_node_as_not_a_page() {
    let v = VaultBuilder::new().file("資料/b.md", "").open();

    let err = v.read_page("資料").unwrap_err();

    assert!(matches!(err, Error::NotAPage(_)), "{err:?}");
}

#[test]
fn read_page_rejects_symlink_to_page_inside_vault_as_not_a_page() {
    let b = VaultBuilder::new().file("a.md", "x");
    let target = b.root().join("a.md");
    let v = b.symlink("link.md", &target).open();

    let err = v.read_page("link.md").unwrap_err();

    assert!(matches!(err, Error::NotAPage(_)), "{err:?}");
}

#[test]
fn read_page_rejects_symlink_to_file_outside_vault_as_not_a_page() {
    let b = VaultBuilder::new().outside_file("secret.md", "外");
    let target = b.outside("secret.md");
    let v = b.symlink("link.md", &target).open();

    let err = v.read_page("link.md").unwrap_err();

    assert!(matches!(err, Error::NotAPage(_)), "{err:?}");
}

#[test]
fn read_page_rejects_page_reached_through_symlinked_folder() {
    let b = VaultBuilder::new().file("real/b.md", "x");
    let target = b.root().join("real");
    let v = b.symlink("linked", &target).open();

    let err = v.read_page("linked/b.md").unwrap_err();

    assert!(matches!(err, Error::NotAPage(_)), "{err:?}");
}

// ---------------------------------------------------------------- 正規形でないパス

fn assert_invalid_path(v: &Vault, path: &str) {
    let err = v.read_page(path).unwrap_err();
    assert!(matches!(err, Error::InvalidPath(_)), "{path:?}: {err:?}");
}

#[test]
fn read_page_rejects_path_escaping_with_parent_dir() {
    let v = VaultBuilder::new().outside_file("secret.md", "外").open();
    assert_invalid_path(&v, "../secret.md");
}

#[test]
fn read_page_rejects_path_escaping_through_inner_parent_dirs() {
    let v = VaultBuilder::new()
        .file("a/x.md", "")
        .outside_file("secret.md", "外")
        .open();
    assert_invalid_path(&v, "a/../../secret.md");
}

#[test]
fn read_page_rejects_parent_dir_even_when_staying_inside_vault() {
    let v = VaultBuilder::new()
        .file("a/x.md", "")
        .file("b.md", "")
        .open();
    assert_invalid_path(&v, "a/../b.md");
}

#[test]
fn read_page_rejects_current_dir_segment() {
    let v = VaultBuilder::new().file("a/b.md", "").open();
    assert_invalid_path(&v, "./a/b.md");
    assert_invalid_path(&v, "a/./b.md");
}

#[test]
fn read_page_rejects_empty_segment() {
    let v = VaultBuilder::new().file("a/b.md", "").open();
    assert_invalid_path(&v, "a//b.md");
    assert_invalid_path(&v, "a/b.md/");
}

#[test]
fn read_page_rejects_empty_path() {
    let v = VaultBuilder::new().file("a.md", "").open();
    assert_invalid_path(&v, "");
}

#[test]
fn read_page_rejects_leading_slash() {
    let v = VaultBuilder::new().file("a.md", "").open();
    assert_invalid_path(&v, "/a.md");
}

#[test]
fn read_page_rejects_absolute_path_outside_vault() {
    let v = VaultBuilder::new().outside_file("secret.md", "外").open();
    assert_invalid_path(&v, v.outside("secret.md").to_str().unwrap());
}

#[test]
fn read_page_rejects_absolute_path_pointing_inside_vault() {
    let v = VaultBuilder::new().file("a.md", "").open();
    assert_invalid_path(&v, v.root().join("a.md").to_str().unwrap());
}

// ================================================================ BOM(回答1)

#[test]
fn read_page_keeps_utf8_bom_in_content() {
    let v = VaultBuilder::new().file("a.md", "\u{FEFF}本文").open();
    assert_eq!(v.read_page("a.md").unwrap().0, "\u{FEFF}本文");
}

#[test]
fn version_differs_between_content_with_and_without_bom() {
    let v = VaultBuilder::new()
        .file("with.md", "\u{FEFF}本文")
        .file("without.md", "本文")
        .open();

    assert_ne!(v.version("with.md"), v.version("without.md"));
}

// ================================================================ 大文字小文字(回答2・16)

#[test]
fn read_page_finds_page_by_path_differing_only_in_letter_case() {
    let v = VaultBuilder::new().file("README.md", "中身").open();
    assert_eq!(v.read_page("readme.md").unwrap().0, "中身");
}

#[test]
fn read_page_finds_page_whose_extension_differs_only_in_letter_case() {
    let v = VaultBuilder::new().file("a.MD", "中身").open();
    assert_eq!(v.read_page("a.md").unwrap().0, "中身");
}

#[test]
fn read_page_finds_page_in_folder_differing_only_in_letter_case() {
    let v = VaultBuilder::new().file("Docs/Guide.md", "中身").open();
    assert_eq!(v.read_page("docs/guide.md").unwrap().0, "中身");
}

#[test]
fn list_tree_merges_page_and_folder_differing_only_in_letter_case() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("A/b.md", "")
        .open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Page);
    assert_eq!(tree[0].path, "a.md");
    assert_eq!(all_paths(&tree[0].children), ["A/b.md".to_string()].into());
}

// ================================================================ 並び順(回答13・14)

#[test]
fn list_tree_sorts_hiragana_before_katakana() {
    let v = VaultBuilder::new()
        .file("ア.md", "")
        .file("あ.md", "")
        .open();
    assert_eq!(titles(&v.list_tree().unwrap()), ["あ", "ア"]);
}

#[test]
fn list_tree_sorts_kanji_by_code_point() {
    // 読み(あ < い)ではなく文字コード(一 U+4E00 < 亜 U+4E9C)の順。
    let v = VaultBuilder::new()
        .file("亜.md", "")
        .file("一.md", "")
        .open();
    assert_eq!(titles(&v.list_tree().unwrap()), ["一", "亜"]);
}

#[test]
fn list_tree_sorts_fullwidth_digits_by_numeric_value() {
    let v = VaultBuilder::new()
        .file("メモ１０.md", "")
        .file("メモ２.md", "")
        .open();
    assert_eq!(titles(&v.list_tree().unwrap()), ["メモ２", "メモ１０"]);
}

#[test]
fn list_tree_sorts_fullwidth_and_halfwidth_digits_together_by_value() {
    let v = VaultBuilder::new()
        .file("メモ１０.md", "")
        .file("メモ3.md", "")
        .file("メモ２.md", "")
        .file("メモ1.md", "")
        .open();

    assert_eq!(
        titles(&v.list_tree().unwrap()),
        ["メモ1", "メモ２", "メモ3", "メモ１０"]
    );
}

#[test]
fn list_tree_breaks_numeric_tie_between_half_and_fullwidth_by_code_point() {
    // 数値として同じなら文字コード順: '2'(U+0032) < '２'(U+FF12)
    let v = VaultBuilder::new()
        .file("メモ２.md", "")
        .file("メモ2.md", "")
        .open();
    assert_eq!(titles(&v.list_tree().unwrap()), ["メモ2", "メモ２"]);
}

// ================================================================ Folder の節点(回答15)

#[test]
fn list_tree_hides_empty_folder() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .dir("空のフォルダ")
        .open();
    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["a.md".to_string()].into()
    );
}

#[test]
fn list_tree_hides_folder_containing_only_non_markdown_files() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("画像/x.png", "png")
        .file("画像/sub/y.txt", "txt")
        .open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["a.md".to_string()].into()
    );
}

#[test]
fn list_tree_hides_folder_containing_only_hidden_pages() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("f/.hidden.md", "")
        .open();
    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["a.md".to_string()].into()
    );
}

#[test]
fn list_tree_hides_folder_containing_only_symlinks() {
    let b = VaultBuilder::new().file("a.md", "").dir("f");
    let target = b.root().join("a.md");
    let v = b.symlink("f/link.md", &target).open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["a.md".to_string()].into()
    );
}

#[test]
fn list_tree_shows_folder_containing_only_non_utf8_page() {
    let v = VaultBuilder::new().file("f/sjis.md", [0x82, 0xA0]).open();
    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["f", "f/sjis.md"].map(String::from).into()
    );
}

#[test]
fn list_tree_shows_every_intermediate_folder_above_a_deep_page() {
    let v = VaultBuilder::new().file("a/b/c.md", "").open();

    let tree = v.list_tree().unwrap();

    let a = find(&tree, "a").unwrap();
    let b = find(&tree, "a/b").unwrap();
    assert_eq!(a.kind, NodeKind::Folder);
    assert_eq!(b.kind, NodeKind::Folder);
    assert_eq!(
        all_paths(&tree),
        ["a", "a/b", "a/b/c.md"].map(String::from).into()
    );
}

#[test]
fn list_tree_page_with_folder_of_only_non_markdown_has_no_children() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/x.png", "png")
        .open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree[0].kind, NodeKind::Page);
    assert!(tree[0].children.is_empty());
}

// ================================================================ index_dir(回答17)

#[test]
fn open_creates_missing_index_dir_including_parents() {
    let b = VaultBuilder::new();
    let index = b.index_dir().join("x/y/index");

    Vault::open(&b.root(), &index).unwrap();

    assert!(index.is_dir());
}

#[test]
fn open_rejects_index_dir_inside_vault() {
    let b = VaultBuilder::new().file("a.md", "").dir("idx");
    let before = all_entries(&b.root());

    let err = Vault::open(&b.root(), &b.root().join("idx")).err().unwrap();

    assert!(matches!(err, Error::IndexOverlapsVault(_)), "{err:?}");
    assert_eq!(all_entries(&b.root()), before);
}

#[test]
fn open_rejects_missing_index_dir_inside_vault_without_creating_it() {
    let b = VaultBuilder::new().file("a.md", "");
    let before = all_entries(&b.root());

    let err = Vault::open(&b.root(), &b.root().join(".index/sub"))
        .err()
        .unwrap();

    assert!(matches!(err, Error::IndexOverlapsVault(_)), "{err:?}");
    assert_eq!(all_entries(&b.root()), before);
}

#[test]
fn open_rejects_index_dir_equal_to_vault_root() {
    let b = VaultBuilder::new().file("a.md", "");
    let before = all_entries(&b.root());

    let err = Vault::open(&b.root(), &b.root()).err().unwrap();

    assert!(matches!(err, Error::IndexOverlapsVault(_)), "{err:?}");
    assert_eq!(all_entries(&b.root()), before);
}

#[test]
fn open_rejects_index_dir_that_is_a_symlink_into_vault() {
    let b = VaultBuilder::new().file("a.md", "").dir("idx");
    let link = b.outside("index-link");
    std::os::unix::fs::symlink(b.root().join("idx"), &link).unwrap();
    let before = all_entries(&b.root());

    let err = Vault::open(&b.root(), &link).err().unwrap();

    assert!(matches!(err, Error::IndexOverlapsVault(_)), "{err:?}");
    assert_eq!(all_entries(&b.root()), before);
}

#[test]
fn open_rejects_vault_inside_index_dir() {
    let index = tempfile::tempdir().unwrap();
    let root = index.path().join("vault");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("a.md"), "").unwrap();
    let before = all_entries(&root);

    let err = Vault::open(&root, index.path()).err().unwrap();

    assert!(matches!(err, Error::IndexOverlapsVault(_)), "{err:?}");
    assert_eq!(all_entries(&root), before);
}

// ================================================================ 名前に使えない文字(回答18)

#[test]
fn read_page_rejects_backslash_in_path() {
    let v = VaultBuilder::new().file("a.md", "").open();
    assert_invalid_path(&v, "a\\b.md");
}

#[test]
fn read_page_rejects_nul_in_path() {
    let v = VaultBuilder::new().file("a.md", "").open();
    assert_invalid_path(&v, "a\0.md");
}

#[test]
fn read_page_rejects_control_characters_in_path() {
    let v = VaultBuilder::new().file("a.md", "").open();
    for path in ["a\n.md", "a\t.md", "a\r.md", "a\u{1}.md", "a\u{1F}.md"] {
        assert_invalid_path(&v, path);
    }
}

#[test]
fn read_page_rejects_existing_file_whose_name_has_backslash() {
    let v = VaultBuilder::new().file("a\\b.md", "x").open();
    assert_invalid_path(&v, "a\\b.md");
}

#[test]
fn list_tree_hides_files_whose_names_have_backslash_or_control_characters() {
    let v = VaultBuilder::new()
        .file("ok.md", "")
        .file("back\\slash.md", "")
        .file("new\nline.md", "")
        .file("tab\t.md", "")
        .file("bell\u{7}.md", "")
        .open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["ok.md".to_string()].into()
    );
}

#[test]
fn list_tree_hides_folder_whose_name_has_control_characters_and_its_pages() {
    let v = VaultBuilder::new()
        .file("ok.md", "")
        .file("bad\tdir.md", "")
        .file("bad\tdir/child.md", "")
        .file("bad\\dir2/child.md", "")
        .open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["ok.md".to_string()].into()
    );
}

#[test]
fn read_page_rejects_del_and_c1_control_characters_in_path() {
    let v = VaultBuilder::new().file("a.md", "").open();
    for path in ["a\u{7F}.md", "a\u{80}.md", "a\u{85}.md", "a\u{9F}.md"] {
        assert_invalid_path(&v, path);
    }
}

#[test]
fn list_tree_hides_files_whose_names_have_del_or_c1_control_characters() {
    let v = VaultBuilder::new()
        .file("ok.md", "")
        .file("del\u{7F}.md", "")
        .file("c1\u{80}.md", "")
        .file("nel\u{85}.md", "")
        .file("c1\u{9F}dir/child.md", "")
        .open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["ok.md".to_string()].into()
    );
}

// ================================================================ 「:」を含む名前(回答21)

#[test]
fn list_tree_shows_page_whose_name_has_colon_as_is() {
    // Finder で「予定/会議」と名付けたファイルは、ディスク上では「予定:会議」になる。
    let v = VaultBuilder::new().file("予定:会議.md", "").open();

    let tree = v.list_tree().unwrap();

    assert_eq!(tree[0].path, "予定:会議.md");
    assert_eq!(tree[0].title, "予定:会議");
}

#[test]
fn list_tree_shows_folder_whose_name_has_colon_and_its_pages() {
    let v = VaultBuilder::new().file("10:00/会議.md", "").open();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["10:00", "10:00/会議.md"].map(String::from).into()
    );
}

#[test]
fn read_page_reads_page_whose_name_has_colon() {
    let v = VaultBuilder::new()
        .file("10:00/予定:会議.md", "中身")
        .open();
    assert_eq!(v.read_page("10:00/予定:会議.md").unwrap().0, "中身");
}
