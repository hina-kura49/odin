//! 段階2の性質テスト(proptest)

mod common;

use common::*;
use odin_core::WriteResult;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// どんな内容でも、書いて読むと変化しない。ディスク上のバイトも内容と一致する。
    #[test]
    fn any_content_survives_write_then_read(original in any::<String>(), content in any::<String>()) {
        let v = VaultBuilder::new().file("a.md", &original).open();
        let base = v.version("a.md");

        let r = v.write_page("a.md", &content, &base).unwrap();

        prop_assert!(matches!(r, WriteResult::Ok { .. }), "{:?}", r);
        prop_assert_eq!(v.disk_bytes("a.md"), content.as_bytes());
        prop_assert_eq!(v.read_page("a.md").unwrap().0, content);
    }

    /// どんな内容でも、読んでそのまま書き戻すとバイト単位で元通り。
    #[test]
    fn read_then_write_back_keeps_bytes(
        text in "(\u{FEFF})?([あ-んA-Za-z0-9 🍣]{0,8}(\r\n|\n|\r|\t)?){0,30}"
    ) {
        let v = VaultBuilder::new().file("a.md", &text).open();

        let (content, version) = v.read_page("a.md").unwrap();
        v.write_page("a.md", &content, &version).unwrap();

        prop_assert_eq!(v.disk_bytes("a.md"), text.as_bytes());
    }

    /// 返された version をつないで何回書いても、すべて成功し、最後の内容が残る。
    #[test]
    fn chained_writes_all_succeed(contents in prop::collection::vec(any::<String>(), 1..8)) {
        let v = VaultBuilder::new().file("a.md", "").open();
        let mut version = v.version("a.md");

        for c in &contents {
            match v.write_page("a.md", c, &version).unwrap() {
                WriteResult::Ok { version: next } => version = next,
                WriteResult::Conflict => prop_assert!(false, "Conflict になった"),
            }
        }

        prop_assert_eq!(v.disk_bytes("a.md"), contents.last().unwrap().as_bytes());
    }

    /// 古い version で書くと、間に入った書き込みが内容を変えていれば必ず Conflict、
    /// 変えていなければ成功する。Conflict のときはファイルが変わらない。
    #[test]
    fn stale_version_conflicts_iff_content_changed(
        original in "[a-cあ]{0,3}", intermediate in "[a-cあ]{0,3}", last in any::<String>()
    ) {
        let v = VaultBuilder::new().file("a.md", &original).open();
        let stale = v.version("a.md");
        v.write_page("a.md", &intermediate, &stale).unwrap();

        let r = v.write_page("a.md", &last, &stale).unwrap();

        if original == intermediate {
            prop_assert!(matches!(r, WriteResult::Ok { .. }), "{:?}", r);
        } else {
            prop_assert_eq!(r, WriteResult::Conflict);
            prop_assert_eq!(v.disk_bytes("a.md"), intermediate.as_bytes());
        }
    }
}
