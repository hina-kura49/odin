//! 段階6: capture_to_inbox

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

/// `content` の中で、`earlier` の最後の行と `later` の最初の行のあいだに、空行がちょうど1つある。
fn assert_one_blank_line_between(content: &str, earlier: &str, later: &str) {
    let joined = format!("{earlier}\n\n{later}");
    assert!(
        content.contains(&joined),
        "{joined:?} が {content:?} に含まれない"
    );
}

// ================================================================ 作る・追記する

#[test]
fn capture_creates_inbox_at_vault_root_when_missing() {
    let v = VaultBuilder::new().file("a.md", "").open();

    v.capture_to_inbox("思いついたこと").unwrap();

    assert!(names_on_disk(&v.root()).contains("Inbox.md"));
    assert!(inbox(&v).contains("思いついたこと"));
}

#[test]
fn created_inbox_does_not_start_with_blank_line() {
    let v = VaultBuilder::new().open();

    v.capture_to_inbox("最初のメモ").unwrap();

    assert!(inbox(&v).starts_with("最初のメモ"), "{:?}", inbox(&v));
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
fn capture_appends_after_existing_content_ending_with_newline() {
    let v = VaultBuilder::new().file("Inbox.md", "既存の行\n").open();

    v.capture_to_inbox("新しいメモ").unwrap();

    let content = inbox(&v);
    assert!(content.starts_with("既存の行\n"), "{content:?}");
    assert_one_blank_line_between(&content, "既存の行", "新しいメモ");
}

#[test]
fn capture_appends_after_existing_content_without_trailing_newline() {
    let v = VaultBuilder::new().file("Inbox.md", "既存の行").open();

    v.capture_to_inbox("新しいメモ").unwrap();

    let content = inbox(&v);
    assert!(content.starts_with("既存の行"), "{content:?}");
    assert_one_blank_line_between(&content, "既存の行", "新しいメモ");
}

#[test]
fn capture_puts_new_text_at_the_end() {
    let v = VaultBuilder::new().file("Inbox.md", "既存\n").open();

    v.capture_to_inbox("最後に来るメモ").unwrap();

    assert!(
        inbox(&v)
            .trim_end_matches(['\n', '\r'])
            .ends_with("最後に来るメモ")
    );
}

#[test]
fn captures_are_appended_in_order_separated_by_blank_lines() {
    let v = VaultBuilder::new().open();

    for text in ["一つ目", "二つ目", "三つ目"] {
        v.capture_to_inbox(text).unwrap();
    }

    let content = inbox(&v);
    assert_one_blank_line_between(&content, "一つ目", "二つ目");
    assert_one_blank_line_between(&content, "二つ目", "三つ目");
}

#[test]
fn capture_keeps_existing_content_byte_for_byte() {
    let existing = "\u{FEFF}---\ntitle: 受信箱\n---\r\n# 見出し\r\n本文  \t\n";
    let v = VaultBuilder::new().file("Inbox.md", existing).open();

    v.capture_to_inbox("追記").unwrap();

    assert!(v.disk_bytes("Inbox.md").starts_with(existing.as_bytes()));
}

#[test]
fn capture_keeps_captured_text_byte_for_byte() {
    let text = "一行目\n二行目 🍣 ＡＢＣ ｶﾀｶﾅ\t末尾の空白  ";
    let v = VaultBuilder::new().file("Inbox.md", "既存\n").open();

    v.capture_to_inbox(text).unwrap();

    assert!(inbox(&v).contains(text));
}

#[test]
fn capture_keeps_nfd_text_as_is() {
    let text = nfd("がっこう");
    let v = VaultBuilder::new().open();

    v.capture_to_inbox(&text).unwrap();

    assert!(inbox(&v).contains(&text));
}

// ================================================================ 既存の Inbox の名前

#[test]
fn capture_appends_to_inbox_whose_name_differs_only_in_case() {
    let v = VaultBuilder::new().file("inbox.md", "既存\n").open();

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(names_on_disk(&v.root()), ["inbox.md".to_string()].into());
    assert!(
        String::from_utf8(v.disk_bytes("inbox.md"))
            .unwrap()
            .contains("追記")
    );
}

#[test]
fn capture_appends_to_inbox_with_uppercase_extension() {
    let v = VaultBuilder::new().file("Inbox.MD", "既存\n").open();

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(names_on_disk(&v.root()), ["Inbox.MD".to_string()].into());
}

#[test]
fn capture_does_not_use_inbox_in_subfolder() {
    let v = VaultBuilder::new()
        .file("p/Inbox.md", "子の受信箱\n")
        .open();

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(v.disk_bytes("p/Inbox.md"), "子の受信箱\n".as_bytes());
    assert!(inbox(&v).contains("追記"));
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

#[test]
fn capture_keeps_file_permissions_of_inbox() {
    let v = VaultBuilder::new().file("Inbox.md", "既存\n").open();
    v.set_mode("Inbox.md", 0o640);

    v.capture_to_inbox("追記").unwrap();

    assert_eq!(v.mode("Inbox.md"), 0o640);
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

// ================================================================ 仕様確認待ち

#[test]
#[ignore = "仕様確認待ち(質問57): 新しく作った Inbox.md の中身は text だけか(末尾に改行を足すか)"]
fn created_inbox_contains_exactly_the_text() {
    let v = VaultBuilder::new().open();
    v.capture_to_inbox("最初のメモ").unwrap();
    assert_eq!(inbox(&v), "最初のメモ");
}

#[test]
#[ignore = "仕様確認待ち(質問58): 空の text や空白だけの text は、何もしないか"]
fn capture_of_empty_text_changes_nothing() {
    let v = VaultBuilder::new().file("Inbox.md", "既存\n").open();
    v.capture_to_inbox("").unwrap();
    assert_eq!(inbox(&v), "既存\n");
}
