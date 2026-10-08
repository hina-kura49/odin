//! 段階6の性質テスト(proptest)

mod common;

use common::*;
use proptest::prelude::*;

/// 確定した追記の手順 (a)〜(e) を、そのまま書いたもの。
/// `existing` は既存の Inbox の中身(なければ None)。戻り値は追記後の中身(何もしないなら None)。
fn expected_after_capture(existing: Option<&str>, text: &str) -> Option<String> {
    // (a) 前後の Unicode の空白を取り除く。(b) 空なら何もしない。
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let existing = existing.unwrap_or("");
    // (c) ない、または0バイトなら「text + LF」(改行がないので LF)。
    if existing.is_empty() {
        return Some(format!("{text}\n"));
    }
    // (e) 最初に現れる改行が CRLF なら CRLF。
    let newline = match existing.find('\n') {
        Some(i) if i > 0 && existing.as_bytes()[i - 1] == b'\r' => "\r\n",
        _ => "\n",
    };
    // (d) 末尾の改行を数える(CRLF は1つ)。
    let mut rest = existing;
    let mut trailing = 0;
    loop {
        if let Some(r) = rest.strip_suffix("\r\n") {
            rest = r;
        } else if let Some(r) = rest.strip_suffix('\n') {
            rest = r;
        } else {
            break;
        }
        trailing += 1;
    }
    let separator = newline.repeat(2usize.saturating_sub(trailing));
    Some(format!("{existing}{separator}{text}{newline}"))
}

/// 取り込む文字列の候補。前後や内側に空白・改行を混ぜる。
const TEXT: &str = "[ \u{3000}\t\n]{0,3}([ぁ-んA-Za-z0-9🍣]([ぁ-んA-Za-z0-9🍣 \u{3000}\t\n]{0,20}[ぁ-んA-Za-z0-9🍣])?)?[ \u{3000}\t\r\n]{0,3}";

/// 既存の中身の候補。LF と CRLF を混ぜ、空行で終わるものも含める。
const EXISTING: &str = "(\u{FEFF})?([ぁ-んa-z]{0,5}(\n|\r\n)?){0,6}";

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// 何回取り込んでも、中身は手順 (a)〜(e) から計算したものと一致する。
    #[test]
    fn captures_follow_the_append_procedure(
        existing in prop::option::of(EXISTING), texts in prop::collection::vec(TEXT, 1..5)
    ) {
        let mut b = VaultBuilder::new();
        if let Some(e) = &existing {
            b = b.file("Inbox.md", e);
        }
        let v = b.open();
        let mut expected = existing.clone();

        for text in &texts {
            v.capture_to_inbox(text).unwrap();
            if let Some(next) = expected_after_capture(expected.as_deref(), text) {
                expected = Some(next);
            }
        }

        match expected {
            Some(e) => prop_assert_eq!(String::from_utf8(v.disk_bytes("Inbox.md")).unwrap(), e),
            None => prop_assert!(!v.exists("Inbox.md"), "空の取り込みだけなのに Inbox.md ができた"),
        }
    }
}
