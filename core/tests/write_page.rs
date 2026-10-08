//! 段階2: write_page(衝突検出、原子的な書き込み、バイトに忠実な保存)

mod common;

use std::io::Read as _;
use std::time::{Duration, SystemTime};

use common::*;
use odin_core::{Error, Vault, WriteResult};

fn t(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

/// write_page が成功したことを確かめ、新しい version を返す。
fn ok_version(r: WriteResult) -> String {
    match r {
        WriteResult::Ok { version } => version,
        WriteResult::Conflict => panic!("Conflict になった"),
    }
}

/// 読んで得た version を base にして書く(ふつうの保存の流れ)。
fn save(v: &Vault, path: &str, content: &str) -> WriteResult {
    let (_, version) = v.read_page(path).unwrap();
    v.write_page(path, content, &version).unwrap()
}

// ================================================================ 保存

#[test]
fn write_page_saves_content_to_disk() {
    let v = VaultBuilder::new().file("a.md", "古い").open();

    ok_version(save(&v, "a.md", "新しい本文\n"));

    assert_eq!(v.disk_bytes("a.md"), "新しい本文\n".as_bytes());
}

#[test]
fn write_page_content_is_returned_by_next_read() {
    let v = VaultBuilder::new().file("a.md", "古い").open();

    ok_version(save(&v, "a.md", "新しい"));

    assert_eq!(v.read_page("a.md").unwrap().0, "新しい");
}

#[test]
fn write_page_returns_version_equal_to_next_read_version() {
    let v = VaultBuilder::new().file("a.md", "古い").open();

    let written = ok_version(save(&v, "a.md", "新しい"));

    assert_eq!(written, v.version("a.md"));
}

#[test]
fn write_page_returns_new_version_different_from_base_when_content_changes() {
    let v = VaultBuilder::new().file("a.md", "古い").open();
    let base = v.version("a.md");

    let written = ok_version(v.write_page("a.md", "新しい", &base).unwrap());

    assert_ne!(written, base);
}

#[test]
fn write_page_with_same_content_succeeds_and_keeps_version() {
    let v = VaultBuilder::new().file("a.md", "同じ").open();
    let base = v.version("a.md");

    let written = ok_version(v.write_page("a.md", "同じ", &base).unwrap());

    assert_eq!(written, base);
}

#[test]
fn write_page_does_not_touch_other_pages() {
    let v = VaultBuilder::new()
        .file("a.md", "A")
        .file("b.md", "B")
        .open();
    v.set_mtime("b.md", t(1_600_000_000));

    ok_version(save(&v, "a.md", "新しい"));

    assert_eq!(v.disk_bytes("b.md"), b"B");
    assert_eq!(mtime(&v.root().join("b.md")), t(1_600_000_000));
}

#[test]
fn write_page_writes_nested_page() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b/c.md", "古い")
        .open();

    ok_version(save(&v, "a/b/c.md", "新しい"));

    assert_eq!(v.disk_bytes("a/b/c.md"), "新しい".as_bytes());
}

// ================================================================ バイトに忠実

#[test]
fn write_page_writes_empty_content_as_zero_bytes() {
    let v = VaultBuilder::new().file("a.md", "中身").open();

    ok_version(save(&v, "a.md", ""));

    assert!(v.disk_bytes("a.md").is_empty());
}

#[test]
fn write_page_preserves_crlf() {
    let v = VaultBuilder::new().file("a.md", "").open();

    ok_version(save(&v, "a.md", "1行目\r\n2行目\r\n"));

    assert_eq!(v.disk_bytes("a.md"), "1行目\r\n2行目\r\n".as_bytes());
}

#[test]
fn write_page_preserves_mixed_line_endings() {
    let v = VaultBuilder::new().file("a.md", "").open();

    ok_version(save(&v, "a.md", "a\r\nb\nc\rd"));

    assert_eq!(v.disk_bytes("a.md"), b"a\r\nb\nc\rd");
}

#[test]
fn write_page_does_not_add_trailing_newline() {
    let v = VaultBuilder::new().file("a.md", "").open();

    ok_version(save(&v, "a.md", "改行なしで終わる"));

    assert_eq!(v.disk_bytes("a.md"), "改行なしで終わる".as_bytes());
}

#[test]
fn write_page_preserves_trailing_spaces_and_blank_lines() {
    let v = VaultBuilder::new().file("a.md", "").open();

    ok_version(save(&v, "a.md", "行末  \n\t\n\n\n"));

    assert_eq!(v.disk_bytes("a.md"), "行末  \n\t\n\n\n".as_bytes());
}

#[test]
fn write_page_preserves_bom_given_in_content() {
    let v = VaultBuilder::new().file("a.md", "").open();

    ok_version(save(&v, "a.md", "\u{FEFF}本文"));

    assert_eq!(v.disk_bytes("a.md"), "\u{FEFF}本文".as_bytes());
}

#[test]
fn write_page_does_not_add_bom_when_content_has_none() {
    let v = VaultBuilder::new().file("a.md", "\u{FEFF}古い").open();

    ok_version(save(&v, "a.md", "新しい"));

    assert_eq!(v.disk_bytes("a.md"), "新しい".as_bytes());
}

#[test]
fn reading_bom_file_and_writing_it_back_keeps_bytes_identical() {
    let original = "\u{FEFF}---\ntitle: x\n---\r\n本文".as_bytes();
    let v = VaultBuilder::new().file("a.md", original).open();

    let (content, version) = v.read_page("a.md").unwrap();
    ok_version(v.write_page("a.md", &content, &version).unwrap());

    assert_eq!(v.disk_bytes("a.md"), original);
}

#[test]
fn write_page_preserves_frontmatter() {
    let v = VaultBuilder::new().file("a.md", "").open();
    let text = "---\ntitle: \"予定: 会議\"\ntags:\n  - a\n  - b\n---\n\n# 本文\n";

    ok_version(save(&v, "a.md", text));

    assert_eq!(v.disk_bytes("a.md"), text.as_bytes());
}

#[test]
fn write_page_preserves_japanese_emoji_and_mixed_width_content() {
    let v = VaultBuilder::new().file("a.md", "").open();
    let text = "絵文字👨‍👩‍👧と国旗🇯🇵、全角ＡＢＣと半角ABC、ｶﾀｶﾅ\u{3000}全角空白";

    ok_version(save(&v, "a.md", text));

    assert_eq!(v.disk_bytes("a.md"), text.as_bytes());
}

#[test]
fn write_page_does_not_normalize_unicode_in_content() {
    let v = VaultBuilder::new().file("a.md", "").open();
    let text = format!("NFD: {} / NFC: {}", nfd("が"), nfc("が"));

    ok_version(save(&v, "a.md", &text));

    assert_eq!(v.disk_bytes("a.md"), text.as_bytes());
}

#[test]
fn write_page_writes_multi_megabyte_content_intact() {
    let v = VaultBuilder::new().file("big.md", "").open();
    let text = "あいうえおabcde🍣\r\n".repeat(200_000); // 約 5MB

    ok_version(save(&v, "big.md", &text));

    assert!(v.disk_bytes("big.md") == text.as_bytes());
}

// ================================================================ 衝突検出

#[test]
fn write_page_returns_conflict_when_file_changed_outside_since_read() {
    let v = VaultBuilder::new().file("a.md", "元").open();
    let base = v.version("a.md");
    v.write_outside_app("a.md", "外で変更");

    let r = v.write_page("a.md", "アプリの変更", &base).unwrap();

    assert_eq!(r, WriteResult::Conflict);
}

#[test]
fn conflict_leaves_file_bytes_unchanged() {
    let v = VaultBuilder::new().file("a.md", "元").open();
    let base = v.version("a.md");
    v.write_outside_app("a.md", "外で変更");

    v.write_page("a.md", "アプリの変更", &base).unwrap();

    assert_eq!(v.disk_bytes("a.md"), "外で変更".as_bytes());
}

#[test]
fn conflict_leaves_file_mtime_unchanged() {
    let v = VaultBuilder::new().file("a.md", "元").open();
    let base = v.version("a.md");
    v.write_outside_app("a.md", "外で変更");
    v.set_mtime("a.md", t(1_600_000_000));

    v.write_page("a.md", "アプリの変更", &base).unwrap();

    assert_eq!(mtime(&v.root().join("a.md")), t(1_600_000_000));
}

#[test]
fn conflict_is_detected_even_when_mtime_is_identical() {
    let v = VaultBuilder::new().file("a.md", "内容A").open();
    v.set_mtime("a.md", t(1_600_000_000));
    let base = v.version("a.md");
    v.write_outside_app("a.md", "内容B"); // 同じバイト長
    v.set_mtime("a.md", t(1_600_000_000));

    let r = v.write_page("a.md", "アプリの変更", &base).unwrap();

    assert_eq!(r, WriteResult::Conflict);
}

#[test]
fn no_conflict_when_only_mtime_changed() {
    let v = VaultBuilder::new().file("a.md", "元").open();
    let base = v.version("a.md");
    v.set_mtime("a.md", t(1_700_000_000));

    let r = v.write_page("a.md", "アプリの変更", &base).unwrap();

    ok_version(r);
    assert_eq!(v.disk_bytes("a.md"), "アプリの変更".as_bytes());
}

#[test]
fn no_conflict_when_file_was_rewritten_outside_with_same_content() {
    let v = VaultBuilder::new().file("a.md", "元").open();
    let base = v.version("a.md");
    v.write_outside_app("a.md", "元");

    ok_version(v.write_page("a.md", "アプリの変更", &base).unwrap());
}

#[test]
fn consecutive_writes_chaining_returned_versions_all_succeed() {
    let v = VaultBuilder::new().file("a.md", "0").open();
    let mut version = v.version("a.md");

    for i in 1..=20 {
        version = ok_version(v.write_page("a.md", &i.to_string(), &version).unwrap());
    }

    assert_eq!(v.disk_bytes("a.md"), b"20");
}

#[test]
fn stale_version_conflicts_even_right_after_own_write() {
    // 同じ1秒内の連続書き込み: 1回目で得た version を使わず、古い base で2回目を書く。
    let v = VaultBuilder::new().file("a.md", "0").open();
    let stale = v.version("a.md");
    ok_version(v.write_page("a.md", "1", &stale).unwrap());

    let r = v.write_page("a.md", "2", &stale).unwrap();

    assert_eq!(r, WriteResult::Conflict);
    assert_eq!(v.disk_bytes("a.md"), b"1");
}

#[test]
fn write_page_with_unknown_base_version_conflicts() {
    let v = VaultBuilder::new().file("a.md", "元").open();

    assert_eq!(
        v.write_page("a.md", "x", "でたらめ").unwrap(),
        WriteResult::Conflict
    );
    assert_eq!(
        v.write_page("a.md", "x", "").unwrap(),
        WriteResult::Conflict
    );
    assert_eq!(v.disk_bytes("a.md"), "元".as_bytes());
}

#[test]
fn version_from_another_page_with_different_content_conflicts() {
    let v = VaultBuilder::new()
        .file("a.md", "A")
        .file("b.md", "B")
        .open();
    let other = v.version("b.md");

    assert_eq!(
        v.write_page("a.md", "x", &other).unwrap(),
        WriteResult::Conflict
    );
}

// ================================================================ 原子的な書き込み

#[test]
fn write_page_leaves_no_extra_files_in_vault() {
    let v = VaultBuilder::new()
        .file("a.md", "古い")
        .file("a/b.md", "")
        .open();
    let before = v.all_entries();

    ok_version(save(&v, "a.md", "新しい"));
    ok_version(save(&v, "a/b.md", "新しい"));

    assert_eq!(v.all_entries(), before);
}

#[test]
fn conflict_leaves_no_extra_files_in_vault() {
    let v = VaultBuilder::new().file("a.md", "古い").open();
    let before = v.all_entries();

    v.write_page("a.md", "x", "でたらめ").unwrap();

    assert_eq!(v.all_entries(), before);
}

#[test]
fn reader_holding_old_file_open_still_sees_complete_old_content() {
    // 一時ファイルに書いてから rename していれば、書き込み前に開いたハンドルは
    // 古い内容を最後まで読める。その場で上書きしていると、途中から新しい内容や空になる。
    let old = "古い内容\n".repeat(10_000);
    let v = VaultBuilder::new().file("a.md", &old).open();
    let mut handle = std::fs::File::open(v.root().join("a.md")).unwrap();

    ok_version(save(&v, "a.md", "新しい"));

    let mut seen = String::new();
    handle.read_to_string(&mut seen).unwrap();
    assert!(seen == old);
}

// ================================================================ ディスク上の名前を保つ

#[test]
fn write_page_via_differently_cased_path_keeps_file_name_case() {
    let v = VaultBuilder::new().file("README.md", "古い").open();
    let base = v.version("readme.md");

    ok_version(v.write_page("readme.md", "新しい", &base).unwrap());

    assert_eq!(names_on_disk(&v.root()), ["README.md".to_string()].into());
    assert_eq!(v.disk_bytes("README.md"), "新しい".as_bytes());
}

#[test]
fn write_page_keeps_uppercase_md_extension() {
    let v = VaultBuilder::new().file("a.MD", "古い").open();
    let base = v.version("a.md");

    ok_version(v.write_page("a.md", "新しい", &base).unwrap());

    assert_eq!(names_on_disk(&v.root()), ["a.MD".to_string()].into());
}

#[test]
fn write_page_via_nfc_path_keeps_nfd_file_name() {
    let disk_name = format!("{}.md", nfd("がっこう"));
    let v = VaultBuilder::new().file(&disk_name, "古い").open();
    let path = nfc("がっこう.md");
    let base = v.version(&path);

    ok_version(v.write_page(&path, "新しい", &base).unwrap());

    assert_eq!(names_on_disk(&v.root()), [disk_name].into());
}

#[test]
fn write_page_via_nfd_path_keeps_nfc_file_name() {
    let disk_name = nfc("がっこう.md");
    let v = VaultBuilder::new().file(&disk_name, "古い").open();
    let path = nfd("がっこう.md");
    let base = v.version(&path);

    ok_version(v.write_page(&path, "新しい", &base).unwrap());

    assert_eq!(names_on_disk(&v.root()), [disk_name].into());
}

// ================================================================ エラー

fn assert_write_err(v: &Vault, path: &str, check: fn(&Error) -> bool) {
    let err = v.write_page(path, "x", "でたらめ").unwrap_err();
    assert!(check(&err), "{path:?}: {err:?}");
}

#[test]
fn write_page_returns_not_found_for_missing_page_and_creates_nothing() {
    let v = VaultBuilder::new().file("a.md", "").open();
    let before = v.all_entries();

    assert_write_err(&v, "none.md", |e| matches!(e, Error::NotFound(_)));
    assert_eq!(v.all_entries(), before);
}

#[test]
fn write_page_returns_not_found_when_parent_folder_is_missing() {
    let v = VaultBuilder::new().file("a.md", "").open();
    let before = v.all_entries();

    assert_write_err(&v, "a/none.md", |e| matches!(e, Error::NotFound(_)));
    assert_eq!(v.all_entries(), before);
}

#[test]
fn write_page_rejects_path_escaping_vault_and_leaves_outside_file_unchanged() {
    let v = VaultBuilder::new().outside_file("secret.md", "外").open();

    assert_write_err(&v, "../secret.md", |e| matches!(e, Error::InvalidPath(_)));
    assert_eq!(
        std::fs::read_to_string(v.outside("secret.md")).unwrap(),
        "外"
    );
}

#[test]
fn write_page_rejects_absolute_path() {
    let v = VaultBuilder::new().outside_file("secret.md", "外").open();
    let abs = v.outside("secret.md");

    assert_write_err(&v, abs.to_str().unwrap(), |e| {
        matches!(e, Error::InvalidPath(_))
    });
    assert_eq!(std::fs::read_to_string(abs).unwrap(), "外");
}

#[test]
fn write_page_rejects_non_canonical_paths() {
    let v = VaultBuilder::new()
        .file("a/b.md", "")
        .file("c.md", "")
        .open();
    for path in [
        "a/../c.md",
        "./c.md",
        "a//b.md",
        "/c.md",
        "",
        "a\\b.md",
        "c\n.md",
        "c\0.md",
    ] {
        assert_write_err(&v, path, |e| matches!(e, Error::InvalidPath(_)));
    }
}

#[test]
fn write_page_rejects_hidden_file_and_leaves_it_unchanged() {
    let v = VaultBuilder::new().file(".hidden.md", "元").open();

    assert_write_err(&v, ".hidden.md", |e| matches!(e, Error::NotAPage(_)));
    assert_eq!(v.disk_bytes(".hidden.md"), "元".as_bytes());
}

#[test]
fn write_page_rejects_non_markdown_file_and_leaves_it_unchanged() {
    let v = VaultBuilder::new().file("notes.txt", "元").open();

    assert_write_err(&v, "notes.txt", |e| matches!(e, Error::NotAPage(_)));
    assert_eq!(v.disk_bytes("notes.txt"), "元".as_bytes());
}

#[test]
fn write_page_rejects_folder_node() {
    let v = VaultBuilder::new().file("資料/b.md", "").open();
    let before = v.all_entries();

    assert_write_err(&v, "資料", |e| matches!(e, Error::NotAPage(_)));
    assert_eq!(v.all_entries(), before);
}

#[test]
fn write_page_rejects_symlink_and_leaves_target_unchanged() {
    let b = VaultBuilder::new().outside_file("secret.md", "外");
    let target = b.outside("secret.md");
    let v = b.symlink("link.md", &target).open();

    assert_write_err(&v, "link.md", |e| matches!(e, Error::NotAPage(_)));
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "外");
    assert!(v.root().join("link.md").is_symlink());
}

#[test]
fn write_page_returns_not_found_for_page_deleted_after_read() {
    let v = VaultBuilder::new().file("a.md", "元").open();
    let base = v.version("a.md");
    std::fs::remove_file(v.root().join("a.md")).unwrap();

    let err = v.write_page("a.md", "x", &base).unwrap_err();

    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
    assert!(!v.root().join("a.md").exists());
}

// ================================================================ UTF-8 でないページ(回答19)

#[test]
fn write_page_rejects_non_utf8_page_whatever_base_version_is() {
    let v = VaultBuilder::new().file("sjis.md", [0x82, 0xA0]).open();
    v.set_mtime("sjis.md", t(1_600_000_000));

    for base in ["", "でたらめ"] {
        let err = v.write_page("sjis.md", "x", base).unwrap_err();
        assert!(matches!(err, Error::NotUtf8(_)), "{base:?}: {err:?}");
    }
    assert_eq!(v.disk_bytes("sjis.md"), [0x82, 0xA0]);
    assert_eq!(mtime(&v.root().join("sjis.md")), t(1_600_000_000));
}

// ================================================================ 権限・拡張属性・作成日(回答20)

#[test]
fn write_page_keeps_file_permissions() {
    for mode in [0o640, 0o664, 0o604] {
        let v = VaultBuilder::new().file("a.md", "古い").open();
        v.set_mode("a.md", mode);

        ok_version(save(&v, "a.md", "新しい"));

        assert_eq!(v.mode("a.md"), mode, "{mode:o}");
    }
}

#[test]
fn write_page_keeps_finder_tags() {
    const TAGS: &str = "com.apple.metadata:_kMDItemUserTags";
    let v = VaultBuilder::new().file("a.md", "古い").open();
    let value = b"bplist00\xa1\x01U\xe8\xb5\xa4\n2\x08\n\x00\x00\x00\x00\x00\x00\x01\x01\x00\x00\x00\x00\x00\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x10";
    xattr::set(v.root().join("a.md"), TAGS, value).unwrap();

    ok_version(save(&v, "a.md", "新しい"));

    assert_eq!(
        xattr::get(v.root().join("a.md"), TAGS).unwrap().as_deref(),
        Some(&value[..])
    );
}

#[test]
fn write_page_keeps_arbitrary_extended_attribute() {
    const NAME: &str = "com.example.odin-test";
    let v = VaultBuilder::new().file("a.md", "古い").open();
    xattr::set(v.root().join("a.md"), NAME, "任意の値".as_bytes()).unwrap();

    ok_version(save(&v, "a.md", "新しい"));

    assert_eq!(
        xattr::get(v.root().join("a.md"), NAME).unwrap().as_deref(),
        Some("任意の値".as_bytes())
    );
}

#[test]
fn write_page_keeps_creation_date() {
    let v = VaultBuilder::new().file("a.md", "古い").open();
    // macOS では、mtime を作成日より前にすると作成日もその時刻になる。
    v.set_mtime("a.md", t(1_500_000_000));
    let created = v.created("a.md");
    assert_eq!(
        created,
        t(1_500_000_000),
        "前提: 作成日を過去にできていない"
    );

    ok_version(save(&v, "a.md", "新しい"));

    assert_eq!(v.created("a.md"), created);
}

#[test]
fn write_page_rejects_read_only_file() {
    let v = VaultBuilder::new().file("a.md", "古い").open();
    let base = v.version("a.md");
    v.set_mode("a.md", 0o444);

    let err = v.write_page("a.md", "新しい", &base).unwrap_err();

    assert!(matches!(err, Error::ReadOnly(_)), "{err:?}");
}

#[test]
fn write_page_leaves_read_only_file_unchanged() {
    let v = VaultBuilder::new().file("a.md", "古い").open();
    let base = v.version("a.md");
    v.set_mode("a.md", 0o444);
    let before = v.all_entries();

    let _ = v.write_page("a.md", "新しい", &base);

    assert_eq!(v.disk_bytes("a.md"), "古い".as_bytes());
    assert_eq!(v.mode("a.md"), 0o444);
    assert_eq!(v.all_entries(), before);
}

// ================================================================ 「:」を含む名前(回答21)

#[test]
fn write_page_writes_page_whose_name_has_colon() {
    let v = VaultBuilder::new()
        .file("10:00/予定:会議.md", "古い")
        .open();

    ok_version(save(&v, "10:00/予定:会議.md", "新しい"));

    assert_eq!(v.disk_bytes("10:00/予定:会議.md"), "新しい".as_bytes());
    assert_eq!(
        names_on_disk(&v.root().join("10:00")),
        ["予定:会議.md".to_string()].into()
    );
}

#[test]
fn write_page_rejects_control_characters_including_del_and_c1() {
    let v = VaultBuilder::new().file("a.md", "").open();
    for path in ["a\u{7F}.md", "a\u{85}.md", "a\u{9F}.md", "a\u{1}.md"] {
        assert_write_err(&v, path, |e| matches!(e, Error::InvalidPath(_)));
    }
}

// ================================================================ 残った一時ファイル

use odin_core::TEMP_FILE_PREFIX as P;

#[test]
fn temp_file_prefix_makes_hidden_non_markdown_names() {
    assert!(P.starts_with('.'));
    assert!(!P.contains('/'));
    assert!(!P.to_lowercase().ends_with(".md"));
}

#[test]
fn open_removes_leftover_temp_files_throughout_vault() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b/c.md", "")
        .file(&format!("{P}1234"), "途中")
        .file(&format!("a/{P}abcd"), "途中")
        .file(&format!("a/b/{P}x"), "途中")
        .open();

    assert_eq!(
        v.all_entries(),
        ["a.md", "a/", "a/b/", "a/b/c.md"].map(String::from).into()
    );
}

#[test]
fn open_keeps_files_whose_names_only_resemble_temp_prefix() {
    let rest = P.strip_prefix('.').unwrap();
    let chopped: String = P.chars().take(P.chars().count() - 1).collect();
    let similar = [
        rest.to_string(),      // 先頭の「.」がない
        format!("{chopped}%"), // 接頭辞の最後の1文字が違う
        format!("x{P}"),       // 途中に接頭辞を含む
        format!(".x{rest}"),   // 「.」と接頭辞の間に別の文字
    ];
    let mut b = VaultBuilder::new().file("a.md", "");
    for name in &similar {
        b = b.file(name, "残す").file(&format!("a/{name}"), "残す");
    }
    let before = all_entries(&b.root());

    let v = b.open();

    assert_eq!(v.all_entries(), before);
}

#[test]
fn open_keeps_hidden_files_in_general() {
    let b = VaultBuilder::new()
        .file("a.md", "")
        .file(".DS_Store", "x")
        .file(".hidden.md", "x")
        .file(".obsidian/workspace.json", "{}")
        .file("a/.gitkeep", "");
    let before = all_entries(&b.root());

    let v = b.open();

    assert_eq!(v.all_entries(), before);
}

#[test]
fn leftover_temp_file_does_not_affect_list_tree() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .open();
    let expected = v.list_tree().unwrap();
    v.write_outside_app(&format!("{P}1"), "途中");
    v.write_outside_app(&format!("a/{P}2"), "途中");
    v.rescan().unwrap();

    assert_eq!(v.list_tree().unwrap(), expected);
}

#[test]
fn leftover_temp_file_does_not_affect_read_page() {
    let v = VaultBuilder::new().file("a.md", "本物").open();
    let expected = v.read_page("a.md").unwrap();
    v.write_outside_app(&format!("{P}1"), "途中");

    assert_eq!(v.read_page("a.md").unwrap(), expected);
}

#[test]
fn leftover_temp_file_is_not_readable_as_page() {
    let v = VaultBuilder::new().file("a.md", "").open();
    let name = format!("{P}1");
    v.write_outside_app(&name, "途中");

    let err = v.read_page(&name).unwrap_err();

    assert!(matches!(err, Error::NotAPage(_)), "{err:?}");
}
