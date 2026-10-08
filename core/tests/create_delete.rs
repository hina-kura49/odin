//! 段階3: create_page / delete_page

mod common;

use common::*;
use odin_core::{Error, NodeKind, PageMeta, Vault};

fn create(v: &Vault, parent: Option<&str>, title: &str) -> PageMeta {
    v.create_page(parent, title).unwrap()
}

// ================================================================ create_page: 基本

#[test]
fn create_page_creates_md_file_at_root() {
    let v = VaultBuilder::new().open();

    let meta = create(&v, None, "買い物");

    assert_eq!(meta.path, "買い物.md");
    assert_eq!(meta.title, "買い物");
    assert!(v.exists("買い物.md"));
}

#[test]
fn create_page_always_uses_lowercase_md_extension() {
    let v = VaultBuilder::new().open();

    create(&v, None, "README");

    assert_eq!(names_on_disk(&v.root()), ["README.md".to_string()].into());
}

#[test]
fn created_page_is_readable_by_returned_path() {
    let v = VaultBuilder::new().open();

    let meta = create(&v, None, "メモ");

    assert!(v.read_page(&meta.path).is_ok());
}

#[test]
fn created_page_appears_in_list_tree() {
    let v = VaultBuilder::new().file("a.md", "").open();

    let meta = create(&v, None, "新しいページ");

    let tree = v.list_tree().unwrap();
    let node = find(&tree, &meta.path).unwrap();
    assert_eq!(node.kind, NodeKind::Page);
    assert_eq!(node.title, "新しいページ");
}

#[test]
fn create_page_returns_modified_at_as_file_mtime_in_milliseconds() {
    let v = VaultBuilder::new().open();

    let meta = create(&v, None, "a");

    assert_eq!(meta.modified_at, v.mtime_ms("a.md"));
}

#[test]
fn create_page_returns_nfc_path_and_title_for_nfd_title() {
    let v = VaultBuilder::new().open();

    let meta = create(&v, None, &nfd("がっこう"));

    assert_eq!(meta.path, nfc("がっこう.md"));
    assert_eq!(meta.title, nfc("がっこう"));
}

#[test]
fn create_page_keeps_japanese_emoji_and_mixed_width_title() {
    let v = VaultBuilder::new().open();
    let title = "メモ📝 ＡＢＣabc １２３ ｶﾀｶﾅ";

    let meta = create(&v, None, title);

    assert_eq!(meta.title, title);
    assert_eq!(meta.path, format!("{title}.md"));
}

#[test]
fn create_page_keeps_fullwidth_colon_and_slash_in_title() {
    let v = VaultBuilder::new().open();

    let meta = create(&v, None, "予定：会議／議事録");

    assert_eq!(meta.title, "予定：会議／議事録");
}

#[test]
fn create_page_treats_md_in_title_as_part_of_title() {
    let v = VaultBuilder::new().open();

    let meta = create(&v, None, "notes.md");

    assert_eq!(meta.title, "notes.md");
    assert_eq!(meta.path, "notes.md.md");
}

#[test]
fn create_page_adds_no_entries_other_than_the_page() {
    let v = VaultBuilder::new().file("a.md", "").open();
    let mut expected = v.all_entries();
    expected.insert("b.md".into());

    create(&v, None, "b");

    assert_eq!(v.all_entries(), expected);
}

// ================================================================ create_page: 子ページ

#[test]
fn create_page_puts_child_in_folder_named_after_parent() {
    let v = VaultBuilder::new().file("仕事.md", "").open();

    let meta = create(&v, Some("仕事.md"), "会議");

    assert_eq!(meta.path, "仕事/会議.md");
    assert!(v.exists("仕事/会議.md"));
}

#[test]
fn create_page_creates_only_parent_folder_and_child() {
    let v = VaultBuilder::new().file("a.md", "").open();
    let mut expected = v.all_entries();
    expected.extend(["a/".to_string(), "a/b.md".to_string()]);

    create(&v, Some("a.md"), "b");

    assert_eq!(v.all_entries(), expected);
}

#[test]
fn create_page_uses_existing_parent_folder() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/x.md", "既存")
        .open();

    let meta = create(&v, Some("a.md"), "y");

    assert_eq!(meta.path, "a/y.md");
    assert_eq!(v.disk_bytes("a/x.md"), "既存".as_bytes());
}

#[test]
fn create_page_nests_under_child_page() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .open();

    let meta = create(&v, Some("a/b.md"), "c");

    assert_eq!(meta.path, "a/b/c.md");
}

#[test]
fn created_child_appears_under_parent_in_list_tree() {
    let v = VaultBuilder::new().file("a.md", "").open();

    create(&v, Some("a.md"), "b");

    let tree = v.list_tree().unwrap();
    assert_eq!(all_paths(&tree[0].children), ["a/b.md".to_string()].into());
}

#[test]
fn create_page_names_new_folder_after_parent_without_uppercase_extension() {
    let v = VaultBuilder::new().file("a.MD", "").open();

    let meta = create(&v, Some("a.MD"), "b");

    assert_eq!(meta.path, "a/b.md");
}

#[test]
fn create_page_names_new_folder_after_parent_as_on_disk_when_parent_given_in_other_case() {
    let v = VaultBuilder::new().file("README.md", "").open();

    let meta = create(&v, Some("readme.md"), "x");

    assert_eq!(meta.path, "README/x.md");
    assert!(names_on_disk(&v.root()).contains("README"));
}

#[test]
fn create_page_uses_existing_folder_whose_case_differs_from_parent() {
    let v = VaultBuilder::new()
        .file("README.md", "")
        .file("readme/old.md", "")
        .open();

    let meta = create(&v, Some("README.md"), "x");

    assert_eq!(meta.path, "readme/x.md");
    assert_eq!(
        names_on_disk(&v.root()),
        ["README.md", "readme"].map(String::from).into()
    );
}

#[test]
fn create_page_accepts_parent_given_in_nfd() {
    let v = VaultBuilder::new().file(&nfc("がっこう.md"), "").open();

    let meta = create(&v, Some(&nfd("がっこう.md")), "子");

    assert_eq!(meta.path, nfc("がっこう/子.md"));
}

#[test]
fn create_page_under_parent_whose_name_has_colon() {
    let v = VaultBuilder::new().file("10:00.md", "").open();

    let meta = create(&v, Some("10:00.md"), "議事録");

    assert_eq!(meta.path, "10:00/議事録.md");
}

// ================================================================ create_page: 名前に使えない文字

#[test]
fn create_page_replaces_slash_in_title_with_hyphen() {
    let v = VaultBuilder::new().open();
    assert_eq!(create(&v, None, "予定/会議").title, "予定-会議");
}

#[test]
fn create_page_replaces_colon_in_title_with_hyphen() {
    let v = VaultBuilder::new().open();
    assert_eq!(create(&v, None, "10:00").title, "10-00");
}

#[test]
fn create_page_replaces_backslash_in_title_with_hyphen() {
    let v = VaultBuilder::new().open();
    assert_eq!(create(&v, None, "a\\b").title, "a-b");
}

#[test]
fn create_page_replaces_control_characters_in_title_with_hyphen() {
    let v = VaultBuilder::new().open();

    let meta = create(&v, None, "a\0b\nc\td\re\u{7F}f\u{85}g\u{9F}h");

    assert_eq!(meta.title, "a-b-c-d-e-f-g-h");
}

#[test]
fn create_page_replaces_each_forbidden_character_separately() {
    let v = VaultBuilder::new().open();
    assert_eq!(create(&v, None, "a//b::c").title, "a--b--c");
}

#[test]
fn create_page_does_not_create_folders_from_slash_in_title() {
    let v = VaultBuilder::new().open();

    create(&v, None, "a/b");

    assert_eq!(v.all_entries(), ["a-b.md".to_string()].into());
}

// ================================================================ create_page: 空のタイトルと連番

#[test]
fn create_page_uses_untitled_for_empty_title() {
    let v = VaultBuilder::new().open();

    let meta = create(&v, None, "");

    assert_eq!(meta.title, "無題");
    assert_eq!(meta.path, "無題.md");
}

#[test]
fn create_page_numbers_duplicate_title_from_two() {
    let v = VaultBuilder::new().file("会議.md", "既存").open();

    let meta = create(&v, None, "会議");

    assert_eq!(meta.title, "会議 2");
    assert_eq!(meta.path, "会議 2.md");
}

#[test]
fn create_page_does_not_overwrite_existing_page() {
    let v = VaultBuilder::new().file("会議.md", "既存").open();

    create(&v, None, "会議");

    assert_eq!(v.disk_bytes("会議.md"), "既存".as_bytes());
}

#[test]
fn create_page_numbers_untitled_pages_in_sequence() {
    let v = VaultBuilder::new().open();

    let titles: Vec<String> = (0..4).map(|_| create(&v, None, "").title).collect();

    assert_eq!(titles, ["無題", "無題 2", "無題 3", "無題 4"]);
}

#[test]
fn create_page_treats_names_differing_only_in_case_as_duplicates() {
    let v = VaultBuilder::new().file("Memo.md", "").open();

    let meta = create(&v, None, "memo");

    assert_eq!(meta.path, "memo 2.md");
}

#[test]
fn create_page_treats_nfc_and_nfd_names_as_duplicates() {
    let v = VaultBuilder::new()
        .file(&format!("{}.md", nfd("が")), "")
        .open();

    let meta = create(&v, None, &nfc("が"));

    assert_eq!(meta.path, nfc("が 2.md"));
}

#[test]
fn create_page_treats_uppercase_md_extension_as_duplicate() {
    let v = VaultBuilder::new().file("a.MD", "").open();

    let meta = create(&v, None, "a");

    assert_eq!(meta.path, "a 2.md");
}

#[test]
fn create_page_numbers_after_replacing_forbidden_characters() {
    let v = VaultBuilder::new().file("a-b.md", "").open();

    let meta = create(&v, None, "a/b");

    assert_eq!(meta.path, "a-b 2.md");
}

#[test]
fn create_page_checks_duplicates_only_in_the_same_folder() {
    let v = VaultBuilder::new().file("a.md", "").file("p.md", "").open();

    let meta = create(&v, Some("p.md"), "a");

    assert_eq!(meta.path, "p/a.md");
}

#[test]
fn create_page_numbers_duplicate_child() {
    let v = VaultBuilder::new()
        .file("p.md", "")
        .file("p/a.md", "")
        .open();

    let meta = create(&v, Some("p.md"), "a");

    assert_eq!(meta.path, "p/a 2.md");
}

// ================================================================ create_page: エラー

fn assert_create_err(v: &TestVault, parent: &str, check: fn(&Error) -> bool) {
    let before = v.all_entries();
    let err = v.create_page(Some(parent), "子").unwrap_err();
    assert!(check(&err), "{parent:?}: {err:?}");
    assert_eq!(v.all_entries(), before, "{parent:?}: 何かが作られた");
}

#[test]
fn create_page_returns_not_found_for_missing_parent() {
    let v = VaultBuilder::new().file("a.md", "").open();
    assert_create_err(&v, "none.md", |e| matches!(e, Error::NotFound(_)));
}

#[test]
fn create_page_returns_not_found_for_parent_deleted_outside_the_app() {
    let v = VaultBuilder::new().file("a.md", "").open();
    std::fs::remove_file(v.root().join("a.md")).unwrap();
    assert_create_err(&v, "a.md", |e| matches!(e, Error::NotFound(_)));
}

#[test]
fn create_page_rejects_non_canonical_parent_path() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .outside_file("secret.md", "外")
        .open();
    for parent in [
        "../secret.md",
        "/a.md",
        "./a.md",
        "a.md/",
        "",
        "a\\b.md",
        "a\n.md",
    ] {
        assert_create_err(&v, parent, |e| matches!(e, Error::InvalidPath(_)));
    }
}

#[test]
fn create_page_rejects_hidden_parent() {
    let v = VaultBuilder::new().file(".hidden.md", "").open();
    assert_create_err(&v, ".hidden.md", |e| matches!(e, Error::NotAPage(_)));
}

#[test]
fn create_page_rejects_non_markdown_parent() {
    let v = VaultBuilder::new().file("notes.txt", "").open();
    assert_create_err(&v, "notes.txt", |e| matches!(e, Error::NotAPage(_)));
}

#[test]
fn create_page_rejects_symlink_parent() {
    let b = VaultBuilder::new().file("a.md", "");
    let target = b.root().join("a.md");
    let v = b.symlink("link.md", &target).open();
    assert_create_err(&v, "link.md", |e| matches!(e, Error::NotAPage(_)));
}

// ================================================================ delete_page: 基本

#[test]
fn delete_page_removes_page_from_vault() {
    let v = VaultBuilder::new().file("a.md", "中身").open();

    v.delete_page("a.md").unwrap();

    assert!(!v.exists("a.md"));
}

#[test]
fn delete_page_moves_page_to_trash_intact() {
    let v = VaultBuilder::new().file("a.md", "中身\r\n").open();

    v.delete_page("a.md").unwrap();

    assert_eq!(v.trashed(), ["a.md".to_string()].into());
    assert_eq!(v.trashed_bytes("a.md"), "中身\r\n".as_bytes());
}

#[test]
fn deleted_page_disappears_from_list_tree() {
    let v = VaultBuilder::new().file("a.md", "").file("b.md", "").open();

    v.delete_page("a.md").unwrap();

    assert_eq!(
        all_paths(&v.list_tree().unwrap()),
        ["b.md".to_string()].into()
    );
}

#[test]
fn read_page_returns_not_found_after_delete() {
    let v = VaultBuilder::new().file("a.md", "").open();
    v.delete_page("a.md").unwrap();

    let err = v.read_page("a.md").unwrap_err();

    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

#[test]
fn delete_page_moves_child_pages_to_trash_together() {
    let v = VaultBuilder::new()
        .file("a.md", "親")
        .file("a/b.md", "子")
        .file("a/b/c.md", "孫")
        .open();

    v.delete_page("a.md").unwrap();

    assert_eq!(v.all_entries(), Default::default());
    assert_eq!(
        v.trashed(),
        ["a.md", "a/", "a/b.md", "a/b/", "a/b/c.md"]
            .map(String::from)
            .into()
    );
    assert_eq!(v.trashed_bytes("a/b/c.md"), "孫".as_bytes());
}

#[test]
fn delete_page_keeps_siblings_and_parent() {
    let v = VaultBuilder::new()
        .file("a.md", "親")
        .file("a/b.md", "消す")
        .file("a/c.md", "残す")
        .file("d.md", "残す")
        .open();

    v.delete_page("a/b.md").unwrap();

    assert_eq!(v.disk_bytes("a.md"), "親".as_bytes());
    assert_eq!(v.disk_bytes("a/c.md"), "残す".as_bytes());
    assert_eq!(v.disk_bytes("d.md"), "残す".as_bytes());
}

#[test]
fn delete_page_does_not_rewrite_links_in_other_pages() {
    let text = "[消すページ](a.md) と [子](a/b.md)\n";
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .file("other.md", text)
        .open();

    v.delete_page("a.md").unwrap();

    assert_eq!(v.disk_bytes("other.md"), text.as_bytes());
}

#[test]
fn delete_page_accepts_path_differing_only_in_case() {
    let v = VaultBuilder::new()
        .file("README.md", "")
        .file("README/x.md", "")
        .open();

    v.delete_page("readme.md").unwrap();

    assert_eq!(v.all_entries(), Default::default());
}

#[test]
fn delete_page_accepts_nfd_path_for_nfc_page() {
    let v = VaultBuilder::new().file(&nfc("がっこう.md"), "").open();

    v.delete_page(&nfd("がっこう.md")).unwrap();

    assert_eq!(v.all_entries(), Default::default());
}

#[test]
fn delete_page_accepts_nfc_path_for_nfd_page_with_children() {
    let v = VaultBuilder::new()
        .file(&format!("{}.md", nfd("がっこう")), "")
        .file(&format!("{}/子.md", nfd("がっこう")), "")
        .open();

    v.delete_page(&nfc("がっこう.md")).unwrap();

    assert_eq!(v.all_entries(), Default::default());
}

#[test]
fn delete_page_deletes_page_with_uppercase_extension_and_its_folder() {
    let v = VaultBuilder::new()
        .file("a.MD", "")
        .file("a/b.md", "")
        .open();

    v.delete_page("a.md").unwrap();

    assert_eq!(v.all_entries(), Default::default());
}

#[test]
fn delete_page_deletes_page_whose_name_has_colon() {
    let v = VaultBuilder::new().file("10:00.md", "").open();

    v.delete_page("10:00.md").unwrap();

    assert!(!v.exists("10:00.md"));
}

#[test]
fn delete_page_leaves_no_extra_files() {
    let v = VaultBuilder::new().file("a.md", "").file("b.md", "").open();

    v.delete_page("a.md").unwrap();

    assert_eq!(v.all_entries(), ["b.md".to_string()].into());
}

// ================================================================ delete_page: エラー

fn assert_delete_err(v: &TestVault, path: &str, check: fn(&Error) -> bool) {
    let before = v.all_entries();
    let err = v.delete_page(path).unwrap_err();
    assert!(check(&err), "{path:?}: {err:?}");
    assert_eq!(v.all_entries(), before, "{path:?}: vault が変わった");
    assert!(v.trashed().is_empty(), "{path:?}: ゴミ箱に移された");
}

#[test]
fn delete_page_returns_not_found_for_missing_page() {
    let v = VaultBuilder::new().file("a.md", "").open();
    assert_delete_err(&v, "none.md", |e| matches!(e, Error::NotFound(_)));
}

#[test]
fn delete_page_returns_not_found_when_deleted_twice() {
    let v = VaultBuilder::new().file("a.md", "").open();
    v.delete_page("a.md").unwrap();

    let err = v.delete_page("a.md").unwrap_err();

    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

#[test]
fn delete_page_rejects_non_canonical_paths() {
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
        "c\u{7F}.md",
    ] {
        assert_delete_err(&v, path, |e| matches!(e, Error::InvalidPath(_)));
    }
    assert!(v.outside("secret.md").exists());
}

#[test]
fn delete_page_rejects_absolute_path() {
    let v = VaultBuilder::new().outside_file("secret.md", "外").open();
    let abs = v.outside("secret.md");
    assert_delete_err(&v, abs.to_str().unwrap(), |e| {
        matches!(e, Error::InvalidPath(_))
    });
    assert!(abs.exists());
}

#[test]
fn delete_page_rejects_hidden_file() {
    let v = VaultBuilder::new().file(".hidden.md", "").open();
    assert_delete_err(&v, ".hidden.md", |e| matches!(e, Error::NotAPage(_)));
}

#[test]
fn delete_page_rejects_non_markdown_file() {
    let v = VaultBuilder::new().file("notes.txt", "").open();
    assert_delete_err(&v, "notes.txt", |e| matches!(e, Error::NotAPage(_)));
}

#[test]
fn delete_page_rejects_folder_node() {
    let v = VaultBuilder::new().file("資料/b.md", "").open();
    assert_delete_err(&v, "資料", |e| matches!(e, Error::NotAPage(_)));
}

#[test]
fn delete_page_rejects_symlink_and_keeps_link_and_target() {
    let b = VaultBuilder::new().outside_file("secret.md", "外");
    let target = b.outside("secret.md");
    let v = b.symlink("link.md", &target).open();

    assert_delete_err(&v, "link.md", |e| matches!(e, Error::NotAPage(_)));
    assert!(target.exists());
}

#[test]
fn delete_page_rejects_leftover_temp_file() {
    let v = VaultBuilder::new().file("a.md", "").open();
    let name = format!("{}1", odin_core::TEMP_FILE_PREFIX);
    v.write_outside_app(&name, "途中");

    assert_delete_err(&v, &name, |e| matches!(e, Error::NotAPage(_)));
}

// ================================================================ 子フォルダは中身ごと(回答23)

#[test]
fn delete_page_moves_whole_child_folder_including_non_pages() {
    let b = VaultBuilder::new()
        .file("a.md", "")
        .file("a/画像.png", "png")
        .file("a/.DS_Store", "x")
        .file("a/sub/.hidden.md", "隠し")
        .outside_file("secret.md", "外");
    let target = b.outside("secret.md");
    let v = b.symlink("a/link.md", &target).open();

    v.delete_page("a.md").unwrap();

    assert_eq!(v.all_entries(), Default::default());
    assert_eq!(
        v.trashed(),
        [
            "a.md",
            "a/",
            "a/画像.png",
            "a/.DS_Store",
            "a/sub/",
            "a/sub/.hidden.md",
            "a/link.md"
        ]
        .map(String::from)
        .into()
    );
}

#[test]
fn delete_page_moves_symlink_itself_without_following_it() {
    let b = VaultBuilder::new()
        .file("a.md", "")
        .dir("a")
        .outside_file("secret.md", "外");
    let target = b.outside("secret.md");
    let v = b.symlink("a/link.md", &target).open();

    v.delete_page("a.md").unwrap();

    assert_eq!(std::fs::read_to_string(&target).unwrap(), "外");
}

// ================================================================ Folder の節点を親に(回答24)

#[test]
fn create_page_under_folder_node_puts_page_in_that_folder() {
    let v = VaultBuilder::new().file("資料/b.md", "").open();

    let meta = create(&v, Some("資料"), "c");

    assert_eq!(meta.path, "資料/c.md");
}

#[test]
fn create_page_under_folder_node_does_not_create_same_named_page() {
    let v = VaultBuilder::new().file("資料/b.md", "").open();
    let mut expected = v.all_entries();
    expected.insert("資料/c.md".into());

    create(&v, Some("資料"), "c");

    assert_eq!(v.all_entries(), expected);
}

#[test]
fn create_page_under_nested_folder_node() {
    let v = VaultBuilder::new().file("a/b/c.md", "").open();

    let meta = create(&v, Some("a/b"), "d");

    assert_eq!(meta.path, "a/b/d.md");
}

#[test]
fn create_page_under_folder_node_given_in_other_case_and_nfd() {
    let v = VaultBuilder::new()
        .file(&format!("{}/x.md", nfc("Docsがく")), "")
        .open();

    let meta = create(&v, Some(&nfd("docsがく")), "y");

    assert_eq!(meta.path, nfc("Docsがく/y.md"));
}

#[test]
fn create_page_returns_not_found_for_missing_folder_parent() {
    let v = VaultBuilder::new().file("a.md", "").open();
    assert_create_err(&v, "none", |e| matches!(e, Error::NotFound(_)));
}

#[test]
fn create_page_rejects_folder_without_pages_as_parent() {
    // ページを含まないフォルダは Folder の節点ではない(回答15)。
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("画像/x.png", "png")
        .open();
    assert_create_err(&v, "画像", |e| matches!(e, Error::NotAPage(_)));
}

// ================================================================ 空になったフォルダ(回答25)

#[test]
fn delete_page_removes_folder_left_empty() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .open();

    v.delete_page("a/b.md").unwrap();

    assert_eq!(v.all_entries(), ["a.md".to_string()].into());
}

#[test]
fn delete_page_removes_empty_ancestor_folders_up_to_root() {
    let v = VaultBuilder::new()
        .file("x.md", "")
        .file("a/b/c/d.md", "")
        .open();

    v.delete_page("a/b/c/d.md").unwrap();

    assert_eq!(v.all_entries(), ["x.md".to_string()].into());
}

#[test]
fn delete_page_stops_removing_at_first_non_empty_ancestor() {
    let v = VaultBuilder::new()
        .file("a/keep.md", "")
        .file("a/b/c/d.md", "")
        .open();

    v.delete_page("a/b/c/d.md").unwrap();

    assert_eq!(
        v.all_entries(),
        ["a/", "a/keep.md"].map(String::from).into()
    );
}

#[test]
fn delete_page_keeps_vault_root_even_when_empty() {
    let v = VaultBuilder::new().file("a/b.md", "").open();

    v.delete_page("a/b.md").unwrap();

    assert!(v.root().is_dir());
    assert_eq!(v.all_entries(), Default::default());
}

#[test]
fn delete_page_keeps_folder_containing_ds_store() {
    let v = VaultBuilder::new()
        .file("a/b.md", "")
        .file("a/.DS_Store", "x")
        .open();

    v.delete_page("a/b.md").unwrap();

    assert_eq!(
        v.all_entries(),
        ["a/", "a/.DS_Store"].map(String::from).into()
    );
}

#[test]
fn delete_page_keeps_folder_containing_attachment() {
    let v = VaultBuilder::new()
        .file("a/b.md", "")
        .file("a/画像.png", "png")
        .open();

    v.delete_page("a/b.md").unwrap();

    assert_eq!(
        v.all_entries(),
        ["a/", "a/画像.png"].map(String::from).into()
    );
}

#[test]
fn delete_page_keeps_folder_containing_empty_subfolder() {
    let v = VaultBuilder::new().file("a/b.md", "").dir("a/空").open();

    v.delete_page("a/b.md").unwrap();

    assert_eq!(v.all_entries(), ["a/", "a/空/"].map(String::from).into());
}

// ================================================================ 新しいページの中身(回答26)

#[test]
fn created_page_is_empty() {
    let v = VaultBuilder::new().open();

    let meta = create(&v, None, "見出しなし");

    assert!(v.disk_bytes(&meta.path).is_empty());
}

// ================================================================ タイトルの処理(回答27)

fn created_title(title: &str) -> String {
    let v = VaultBuilder::new().open();
    create(&v, None, title).title
}

#[test]
fn create_page_trims_unicode_whitespace_around_title() {
    assert_eq!(
        created_title(" \u{3000}\t\n\u{2003}会議\u{3000} \r\n"),
        "会議"
    );
}

#[test]
fn create_page_keeps_whitespace_inside_title() {
    assert_eq!(
        created_title("朝の\u{3000}会議 メモ"),
        "朝の\u{3000}会議 メモ"
    );
}

#[test]
fn create_page_uses_untitled_for_whitespace_only_title() {
    assert_eq!(created_title(" \u{3000}\t\n"), "無題");
}

#[test]
fn create_page_trims_before_replacing_forbidden_characters() {
    // 端の改行は取り除かれ、内側の改行だけが「-」になる。
    assert_eq!(created_title("\na\nb\n"), "a-b");
}

#[test]
fn create_page_replaces_single_dot_title_with_hyphen() {
    assert_eq!(created_title("."), "-");
}

#[test]
fn create_page_replaces_double_dot_title_with_two_hyphens() {
    assert_eq!(created_title(".."), "--");
}

#[test]
fn create_page_replaces_leading_dot_so_page_is_not_hidden() {
    let v = VaultBuilder::new().open();

    let meta = create(&v, None, ".env");

    assert_eq!(meta.title, "-env");
    assert_eq!(meta.path, "-env.md");
}

#[test]
fn create_page_replaces_every_leading_dot() {
    assert_eq!(created_title("...a.b"), "---a.b");
}

#[test]
fn create_page_keeps_dots_not_at_the_start() {
    assert_eq!(created_title("v1.2."), "v1.2.");
}

#[test]
fn create_page_replaces_dot_that_becomes_leading_after_trim() {
    assert_eq!(created_title("  .env  "), "-env");
}

#[test]
fn create_page_keeps_title_of_exactly_200_bytes() {
    let title = "a".repeat(200);
    assert_eq!(created_title(&title), title);
}

#[test]
fn create_page_truncates_title_over_200_bytes() {
    // 「あ」は3バイト。101文字 = 303バイト → 66文字 = 198バイト
    assert_eq!(created_title(&"あ".repeat(101)), "あ".repeat(66));
}

#[test]
fn create_page_truncates_at_grapheme_boundary() {
    // 家族の絵文字は複数の文字からなる1つの書記素(18バイト)。途中で切らない。
    let family = "👨\u{200D}👩\u{200D}👧";
    let title = format!("{}{family}", "a".repeat(190));

    assert_eq!(created_title(&title), "a".repeat(190));
}

#[test]
fn create_page_trims_again_after_truncation() {
    let title = format!("{} bb", "a".repeat(199)); // 202バイト → 200バイトで切ると末尾が空白
    assert_eq!(created_title(&title), "a".repeat(199));
}

#[test]
fn create_page_measures_length_after_nfc_normalization() {
    // NFD の「が」は6バイト、NFC では3バイト。67文字は NFC で201バイトなので66文字に切る。
    assert_eq!(created_title(&nfd(&"が".repeat(67))), nfc(&"が".repeat(66)));
}

#[test]
fn create_page_measures_length_after_replacing_forbidden_characters() {
    // 「:」(1バイト)は「-」(1バイト)になり長さは変わらないが、処理後の文字列で数える。
    let title = format!("{}:", "a".repeat(199));
    assert_eq!(created_title(&title), format!("{}-", "a".repeat(199)));
}

#[test]
fn create_page_numbers_truncated_title_when_it_collides() {
    let v = VaultBuilder::new().open();
    let long = "a".repeat(250);
    create(&v, None, &long);

    let meta = create(&v, None, &long);

    assert_eq!(meta.title, format!("{} 2", "a".repeat(200)));
}

// ================================================================ 連番(回答28)

#[test]
fn create_page_uses_smallest_free_number_from_two() {
    let v = VaultBuilder::new()
        .file("無題.md", "")
        .file("無題 3.md", "")
        .open();

    assert_eq!(create(&v, None, "").title, "無題 2");
}

#[test]
fn create_page_uses_plain_title_when_only_numbered_ones_exist() {
    let v = VaultBuilder::new().file("無題 2.md", "").open();

    assert_eq!(create(&v, None, "").title, "無題");
}

#[test]
fn create_page_does_not_interpret_trailing_number_in_title() {
    let v = VaultBuilder::new().file("無題 2.md", "").open();

    assert_eq!(create(&v, None, "無題 2").title, "無題 2 2");
}

#[test]
fn create_page_numbers_title_with_trailing_number_using_smallest_free() {
    let v = VaultBuilder::new()
        .file("会議 2.md", "")
        .file("会議 2 2.md", "")
        .open();

    assert_eq!(create(&v, None, "会議 2").title, "会議 2 3");
}

// ================================================================ Folder の節点と同名(回答29)

#[test]
fn create_page_does_not_count_folder_node_as_duplicate() {
    let v = VaultBuilder::new().file("a/b.md", "").open();

    let meta = create(&v, None, "a");

    assert_eq!(meta.path, "a.md");
}

#[test]
fn page_created_beside_same_named_folder_adopts_its_contents() {
    let v = VaultBuilder::new().file("a/b.md", "").open();

    create(&v, None, "a");

    let tree = v.list_tree().unwrap();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Page);
    assert_eq!(tree[0].path, "a.md");
    assert_eq!(all_paths(&tree[0].children), ["a/b.md".to_string()].into());
}

#[test]
fn create_page_does_not_count_folder_with_different_case_as_duplicate() {
    let v = VaultBuilder::new().file("A/b.md", "").open();

    let meta = create(&v, None, "a");

    assert_eq!(meta.path, "a.md");
}

// ================================================================ UTF-8 でない・読み取り専用(回答30)

#[test]
fn delete_page_deletes_non_utf8_page() {
    let v = VaultBuilder::new().file("sjis.md", [0x82, 0xA0]).open();

    v.delete_page("sjis.md").unwrap();

    assert_eq!(v.trashed(), ["sjis.md".to_string()].into());
}

#[test]
fn delete_page_deletes_read_only_page() {
    let v = VaultBuilder::new().file("a.md", "").open();
    v.set_mode("a.md", 0o444);

    v.delete_page("a.md").unwrap();

    assert_eq!(v.trashed(), ["a.md".to_string()].into());
}

#[test]
fn create_page_accepts_non_utf8_page_as_parent_without_changing_it() {
    let v = VaultBuilder::new().file("sjis.md", [0x82, 0xA0]).open();

    let meta = create(&v, Some("sjis.md"), "子");

    assert_eq!(meta.path, "sjis/子.md");
    assert_eq!(v.disk_bytes("sjis.md"), [0x82, 0xA0]);
}

#[test]
fn create_page_accepts_read_only_page_as_parent_without_changing_it() {
    let v = VaultBuilder::new().file("a.md", "親").open();
    v.set_mode("a.md", 0o444);

    let meta = create(&v, Some("a.md"), "子");

    assert_eq!(meta.path, "a/子.md");
    assert_eq!(v.disk_bytes("a.md"), "親".as_bytes());
    assert_eq!(v.mode("a.md"), 0o444);
}

// ================================================================ ゴミ箱への移動の失敗(回答31)

#[test]
fn delete_page_returns_io_error_when_moving_child_folder_fails() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .trash_fails_on_call(1)
        .open();

    let err = v.delete_page("a.md").unwrap_err();

    assert!(matches!(err, Error::Io(_)), "{err:?}");
}

#[test]
fn failed_child_folder_move_leaves_vault_and_trash_untouched() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .trash_fails_on_call(1)
        .open();
    let before = v.all_entries();

    let _ = v.delete_page("a.md");

    assert_eq!(v.all_entries(), before);
    assert!(v.trashed().is_empty());
}

#[test]
fn delete_page_returns_io_error_when_moving_page_after_child_folder_fails() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .trash_fails_on_call(2)
        .open();

    let err = v.delete_page("a.md").unwrap_err();

    assert!(matches!(err, Error::Io(_)), "{err:?}");
}

#[test]
fn failed_page_move_leaves_page_readable_and_children_in_trash() {
    let v = VaultBuilder::new()
        .file("a.md", "親")
        .file("a/b.md", "子")
        .trash_fails_on_call(2)
        .open();

    let _ = v.delete_page("a.md");

    assert_eq!(v.read_page("a.md").unwrap().0, "親");
    assert_eq!(v.all_entries(), ["a.md".to_string()].into());
    assert_eq!(v.trashed(), ["a/", "a/b.md"].map(String::from).into());
}

#[test]
fn failed_move_of_page_without_children_leaves_vault_untouched() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .trash_fails_on_call(1)
        .open();

    let err = v.delete_page("a.md").unwrap_err();

    assert!(matches!(err, Error::Io(_)), "{err:?}");
    assert_eq!(v.all_entries(), ["a.md".to_string()].into());
}

// ================================================================ 同名のファイルが邪魔をする(回答32)

#[test]
fn create_page_returns_name_occupied_when_file_blocks_child_folder() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a", "拡張子なし")
        .open();
    assert_create_err(&v, "a.md", |e| matches!(e, Error::NameOccupied(_)));
}

#[test]
fn create_page_leaves_blocking_file_untouched() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a", "拡張子なし")
        .open();

    let _ = v.create_page(Some("a.md"), "子");

    assert_eq!(v.disk_bytes("a"), "拡張子なし".as_bytes());
}

#[test]
fn create_page_returns_name_occupied_when_file_differing_in_case_blocks_child_folder() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("A", "拡張子なし")
        .open();
    assert_create_err(&v, "a.md", |e| matches!(e, Error::NameOccupied(_)));
}
