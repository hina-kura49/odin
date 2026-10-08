//! 段階6の性質テスト(proptest)

mod common;

use common::*;
use proptest::prelude::*;

/// 取り込む文字列の候補。前後に空白や改行を置かない(扱いは仕様確認待ち)。
/// 既存の中身も、空行で終わるものは作らない(扱いは仕様確認待ち)。
const TEXT: &str = "[ぁ-んA-Za-z0-9🍣]([ぁ-んA-Za-z0-9🍣 \n]{0,20}[ぁ-んA-Za-z0-9🍣])?";

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// 既存の中身は先頭にそのまま残り、取り込んだ文字列は順に、空行1つを挟んで並ぶ。
    #[test]
    fn captures_keep_existing_content_and_order(
        existing in prop::option::of("[ぁ-んa-z]([ぁ-んa-z\n]{0,28}[ぁ-んa-z])?\n?"), texts in prop::collection::vec(TEXT, 1..5)
    ) {
        let mut b = VaultBuilder::new();
        if let Some(e) = &existing {
            b = b.file("Inbox.md", e);
        }
        let v = b.open();

        for text in &texts {
            v.capture_to_inbox(text).unwrap();
        }

        let content = String::from_utf8(v.disk_bytes("Inbox.md")).unwrap();
        if let Some(e) = &existing {
            prop_assert!(content.starts_with(e.as_str()));
        }
        for pair in texts.windows(2) {
            let joined = format!("{}\n\n{}", pair[0], pair[1]);
            prop_assert!(content.contains(&joined), "{joined:?} が {content:?} に含まれない");
        }
        let last = texts.last().unwrap();
        prop_assert!(content.trim_end_matches('\n').ends_with(last.as_str()));
    }
}
