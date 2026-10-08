//! 段階6: capture_to_inbox
//!
//! 追記の手順(確定した仕様):
//! (a) text の前後の空白と改行(Unicode の空白すべて)を取り除く。内側は変えない。
//! (b) 結果が空なら何もせず成功。ファイルを作らず、既存の中身も mtime も変えない。
//! (c) Inbox がない、または0バイトなら、中身を「text + 改行」にする。
//! (d) それ以外は既存の中身の後ろに、末尾の改行が0個なら改行2つ、1個なら1つ、2個以上なら何も足さず、
//!     続けて「text + 改行」。
//! (e) 足す改行は、既存の中身に最初に現れる改行が CRLF なら CRLF、それ以外は LF。
//!     末尾の改行を数えるときは CRLF を1つと数える。

mod common;

use std::time::{Duration, SystemTime};

use common::*;
use odin_core::{Error, NodeKind, WriteResult};

fn t(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

fn inbox(v: &TestVault) -> String {
    String::from_utf8(v.disk_bytes("Inbox.md")).unwrap()
}

/// 既存の Inbox が `existing` のときに `text` を取り込んだ結果を確かめる。
fn assert_capture(existing: &str, text: &str, expected: &str) {
    let v = VaultBuilder::new().file("Inbox.md", existing).open();

    v.capture_to_inbox(text).unwrap();

    assert_eq!(inbox(&v), expected, "既存 {existing:?} に {text:?}");
}

// ================================================================ (c) 新しく作る・0バイト

#[test]
fn capture_creates_inbox_with_text_and_newline() {
    let v = VaultBuilder::new().file("a.md", "").open();

    v.capture_to_inbox("思いついたこと").unwrap();

    assert_eq!(inbox(&v), "思いついたこと\n");
}

#[test]
fn capture_creates_inbox_at_vault_root() {
    let v = VaultBuilder::new().file("p/a.md", "").open();

    v.capture_to_inbox("x").unwrap();

    assert!(names_on_disk(&v.root()).contains("Inbox.md"));
}

#[test]
fn created_inbox_is_a_page_in_list_tree() {
    let v = VaultBuilder::new().open();

    v.capture_to_inbox("x").unwrap();

    let tree = v.list_tree().unwrap();
    assert_eq!(tree[0].path, "Inbox.md");
    assert_eq!(tree[0].kind, NodeKind::Page);
}

#[test]
fn capture_into_empty_inbox_writes_text_and_newline() {
    assert_capture("", "メモ", "メモ\n");
}

// ================================================================ (d) 末尾の改行の数

#[test]
fn capture_adds_two_newlines_when_existing_does_not_end_with_newline() {
    assert_capture("既存", "メモ", "既存\n\nメモ\n");
}

#[test]
fn capture_adds_one_newline_when_existing_ends_with_one_newline() {
    assert_capture("既存\n", "メモ", "既存\n\nメモ\n");
}

#[test]
fn capture_adds_no_newline_when_existing_ends_with_blank_line() {
    assert_capture("既存\n\n", "メモ", "既存\n\nメモ\n");
}

#[test]
fn capture_does_not_remove_extra_blank_lines_at_end() {
    assert_capture("既存\n\n\n\n", "メモ", "既存\n\n\n\nメモ\n");
}

#[test]
fn captures_are_appended_in_order() {
    let v = VaultBuilder::new().open();

    for text in ["一つ目", "二つ目", "三つ目"] {
        v.capture_to_inbox(text).unwrap();
    }

    assert_eq!(inbox(&v), "一つ目\n\n二つ目\n\n三つ目\n");
}

#[test]
fn capture_keeps_existing_content_byte_for_byte() {
    let existing = "\u{FEFF}---\ntitle: 受信箱\n---\n# 見出し\n本文  \t";
    assert_capture(existing, "追記", &format!("{existing}\n\n追記\n"));
}

#[test]
fn capture_treats_bom_only_inbox_as_non_empty() {
    // BOM だけのファイルは0バイトではないので (d) に当たる。
    assert_capture("\u{FEFF}", "メモ", "\u{FEFF}\n\nメモ\n");
}

// ================================================================ (e) 改行の種類

#[test]
fn capture_uses_crlf_when_first_newline_is_crlf() {
    assert_capture(
        "一行目\r\n二行目",
        "メモ",
        "一行目\r\n二行目\r\n\r\nメモ\r\n",
    );
}

#[test]
fn capture_counts_trailing_crlf_as_one_newline() {
    assert_capture("既存\r\n", "メモ", "既存\r\n\r\nメモ\r\n");
}

#[test]
fn capture_counts_two_trailing_crlfs_as_blank_line() {
    assert_capture("既存\r\n\r\n", "メモ", "既存\r\n\r\nメモ\r\n");
}

#[test]
fn capture_uses_lf_when_first_newline_is_lf_even_if_later_ones_are_crlf() {
    assert_capture("一行目\n二行目\r\n", "メモ", "一行目\n二行目\r\n\nメモ\n");
}

#[test]
fn capture_uses_crlf_when_first_newline_is_crlf_even_if_later_ones_are_lf() {
    assert_capture(
        "一行目\r\n二行目\n",
        "メモ",
        "一行目\r\n二行目\n\r\nメモ\r\n",
    );
}

#[test]
fn capture_uses_lf_when_existing_has_no_newline() {
    assert_capture("改行なし", "メモ", "改行なし\n\nメモ\n");
}

#[test]
fn capture_keeps_newlines_inside_text_as_is_even_in_crlf_inbox() {
    assert_capture("既存\r\n", "一\n二", "既存\r\n\r\n一\n二\r\n");
}

// ================================================================ (a) 前後の空白を取り除く

#[test]
fn capture_trims_unicode_whitespace_around_text() {
    assert_capture(
        "既存\n",
        " \u{3000}\t\n\r\nメモ\u{3000} \n\n",
        "既存\n\nメモ\n",
    );
}

#[test]
fn capture_keeps_whitespace_inside_text() {
    assert_capture(
        "既存\n",
        "  一行目  \n\n\t二行目\u{3000}三  ",
        "既存\n\n一行目  \n\n\t二行目\u{3000}三\n",
    );
}

#[test]
fn capture_keeps_text_byte_for_byte_inside() {
    let text = format!("絵文字🍣 ＡＢＣ ｶﾀｶﾅ {}", nfd("がっこう"));
    assert_capture("既存\n", &text, &format!("既存\n\n{text}\n"));
}

// ================================================================ (b) 空なら何もしない

#[test]
fn capture_of_empty_text_does_not_create_inbox() {
    let v = VaultBuilder::new().file("a.md", "").open();

    v.capture_to_inbox("").unwrap();
    v.capture_to_inbox(" \u{3000}\t\n\r\n").unwrap();

    assert_eq!(v.all_entries(), ["a.md".to_string()].into());
}

#[test]
fn capture_of_whitespace_only_text_leaves_existing_inbox_unchanged() {
    let v = VaultBuilder::new()
        .file("Inbox.md", "既存")
        .mtime("Inbox.md", t(1_600_000_000))
        .open();

    v.capture_to_inbox(" \u{3000}\n").unwrap();

    assert_eq!(inbox(&v), "既存");
    assert_eq!(mtime(&v.root().join("Inbox.md")), t(1_600_000_000));
}

#[test]
fn capture_of_empty_text_succeeds_even_if_inbox_is_read_only() {
    let v = VaultBuilder::new().file("Inbox.md", "既存").open();
    v.set_mode("Inbox.md", 0o444);

    assert!(v.capture_to_inbox("  ").is_ok());
}

// ================================================================ 既存の Inbox の名前

#[test]
fn capture_appends_to_inbox_whose_name_differs_only_in_case() {
    let v = VaultBuilder::new().file("inbox.md", "既存\n").open();

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(names_on_disk(&v.root()), ["inbox.md".to_string()].into());
    assert_eq!(v.disk_bytes("inbox.md"), "既存\n\n追記\n".as_bytes());
}

#[test]
fn capture_appends_to_inbox_with_uppercase_extension() {
    let v = VaultBuilder::new().file("Inbox.MD", "既存\n").open();

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(names_on_disk(&v.root()), ["Inbox.MD".to_string()].into());
    assert_eq!(v.disk_bytes("Inbox.MD"), "既存\n\n追記\n".as_bytes());
}

#[test]
fn capture_does_not_use_inbox_in_subfolder() {
    let v = VaultBuilder::new()
        .file("p/Inbox.md", "子の受信箱\n")
        .open();

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(v.disk_bytes("p/Inbox.md"), "子の受信箱\n".as_bytes());
    assert_eq!(inbox(&v), "追記\n");
}

#[test]
fn capture_creates_inbox_beside_inbox_folder_and_adopts_it() {
    let v = VaultBuilder::new().file("Inbox/古いメモ.md", "").open();

    v.capture_to_inbox("追記").unwrap();

    let tree = v.list_tree().unwrap();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Page);
    assert_eq!(tree[0].path, "Inbox.md");
    assert_eq!(
        all_paths(&tree[0].children),
        ["Inbox/古いメモ.md".to_string()].into()
    );
}

// ================================================================ ほかを変えない・反映する

#[test]
fn capture_touches_no_other_file() {
    let v = VaultBuilder::new()
        .file("a.md", "A")
        .file("p/b.md", "B")
        .mtime("a.md", t(1_600_000_000))
        .open();

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(v.disk_bytes("a.md"), b"A");
    assert_eq!(mtime(&v.root().join("a.md")), t(1_600_000_000));
    assert_eq!(
        v.all_entries(),
        ["Inbox.md", "a.md", "p/", "p/b.md"]
            .map(String::from)
            .into()
    );
}

#[test]
fn capture_leaves_no_temp_files() {
    let v = VaultBuilder::new().file("Inbox.md", "既存\n").open();

    v.capture_to_inbox("一").unwrap();
    v.capture_to_inbox("二").unwrap();

    assert_eq!(v.all_entries(), ["Inbox.md".to_string()].into());
}

#[test]
fn capture_is_reflected_in_search_immediately() {
    let v = VaultBuilder::new().file("Inbox.md", "既存\n").open();

    v.capture_to_inbox("取り込んだ合言葉").unwrap();

    let hits: Vec<String> = v
        .search("取り込んだ合言葉", 10)
        .unwrap()
        .into_iter()
        .map(|h| h.path)
        .collect();
    assert_eq!(hits, ["Inbox.md"]);
}

#[test]
fn capture_is_reflected_in_recent_pages_immediately() {
    let v = VaultBuilder::new()
        .file("Inbox.md", "既存\n")
        .file("a.md", "")
        .mtime("Inbox.md", t(1_600_000_000))
        .mtime("a.md", t(1_700_000_000))
        .open();

    v.capture_to_inbox("追記").unwrap();

    let recent: Vec<String> = v
        .recent_pages(10)
        .unwrap()
        .into_iter()
        .map(|m| m.path)
        .collect();
    assert_eq!(recent, ["Inbox.md", "a.md"]);
}

#[test]
fn editor_holding_old_version_of_inbox_gets_conflict_after_capture() {
    let v = VaultBuilder::new().file("Inbox.md", "既存\n").open();
    let (_, old_version) = v.read_page("Inbox.md").unwrap();

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(
        v.write_page("Inbox.md", "編集中の内容", &old_version)
            .unwrap(),
        WriteResult::Conflict
    );
}

// ================================================================ 権限・拡張属性・作成日(回答68: write_page と同じ)

#[test]
fn capture_keeps_file_permissions_of_inbox() {
    let v = VaultBuilder::new().file("Inbox.md", "既存\n").open();
    v.set_mode("Inbox.md", 0o640);

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(v.mode("Inbox.md"), 0o640);
}

#[test]
fn capture_keeps_extended_attributes_of_inbox() {
    const TAGS: &str = "com.apple.metadata:_kMDItemUserTags";
    const OTHER: &str = "com.example.odin-test";
    let v = VaultBuilder::new().file("Inbox.md", "既存\n").open();
    let path = v.root().join("Inbox.md");
    xattr::set(&path, TAGS, b"bplist00tags").unwrap();
    xattr::set(&path, OTHER, "任意の値".as_bytes()).unwrap();

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(
        xattr::get(&path, TAGS).unwrap().as_deref(),
        Some(&b"bplist00tags"[..])
    );
    assert_eq!(
        xattr::get(&path, OTHER).unwrap().as_deref(),
        Some("任意の値".as_bytes())
    );
}

#[test]
fn capture_keeps_creation_date_of_inbox() {
    let v = VaultBuilder::new().file("Inbox.md", "既存\n").open();
    // macOS では、mtime を作成日より前にすると作成日もその時刻になる。
    v.set_mtime("Inbox.md", t(1_500_000_000));
    let created = v.created("Inbox.md");
    assert_eq!(
        created,
        t(1_500_000_000),
        "前提: 作成日を過去にできていない"
    );

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(v.created("Inbox.md"), created);
}

// ================================================================ 書けない Inbox

#[test]
fn capture_returns_read_only_for_read_only_inbox_and_leaves_it_unchanged() {
    let v = VaultBuilder::new().file("Inbox.md", "既存\n").open();
    v.set_mode("Inbox.md", 0o444);

    let err = v.capture_to_inbox("追記").unwrap_err();

    assert!(matches!(err, Error::ReadOnly(_)), "{err:?}");
    assert_eq!(v.disk_bytes("Inbox.md"), "既存\n".as_bytes());
}

#[test]
fn capture_returns_not_utf8_for_non_utf8_inbox_and_leaves_it_unchanged() {
    let v = VaultBuilder::new().file("Inbox.md", [0x82, 0xA0]).open();

    let err = v.capture_to_inbox("追記").unwrap_err();

    assert!(matches!(err, Error::NotUtf8(_)), "{err:?}");
    assert_eq!(v.disk_bytes("Inbox.md"), [0x82, 0xA0]);
}

#[test]
fn capture_rejects_inbox_that_is_a_symlink_and_leaves_target_unchanged() {
    let b = VaultBuilder::new().outside_file("outside.md", "外");
    let target = b.outside("outside.md");
    let v = b.symlink("Inbox.md", &target).open();

    let err = v.capture_to_inbox("追記").unwrap_err();

    assert!(matches!(err, Error::NotAPage(_)), "{err:?}");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "外");
    assert!(v.root().join("Inbox.md").is_symlink());
}
