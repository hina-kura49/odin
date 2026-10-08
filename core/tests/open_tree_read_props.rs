//! 段階1の性質テスト(proptest)

mod common;

use common::*;
use proptest::prelude::*;

/// ファイル名として安全な名前(先頭は `.` や空白にしない)。
const SAFE_NAME: &str = "[a-zA-Z0-9ぁ-んァ-ン一-龠ー_][a-zA-Z0-9ぁ-んァ-ン一-龠ー_ .-]{0,19}";

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// ディスク上のどんな UTF-8 テキストも、read_page でそのまま返る。
    #[test]
    fn read_page_returns_any_utf8_text_unchanged(text in any::<String>()) {
        let v = VaultBuilder::new().file("a.md", &text).open();
        prop_assert_eq!(v.read_page("a.md").unwrap().0, text);
    }

    /// 改行コードや空白が混在していても、read_page でそのまま返る。
    #[test]
    fn read_page_returns_mixed_line_endings_unchanged(
        text in "([あ-んA-Za-z ]{0,8}(\r\n|\n|\r| |\t)?){0,50}"
    ) {
        let v = VaultBuilder::new().file("a.md", &text).open();
        prop_assert_eq!(v.read_page("a.md").unwrap().0, text);
    }

    /// 内容が同じなら version は同じ、違えば違う。
    #[test]
    fn version_is_equal_iff_content_is_equal(a in any::<String>(), b in any::<String>()) {
        let v = VaultBuilder::new().file("a.md", &a).file("b.md", &b).open();
        let (_, va) = v.read_page("a.md").unwrap();
        let (_, vb) = v.read_page("b.md").unwrap();
        prop_assert_eq!(a == b, va == vb);
    }

    /// タイトルは拡張子を除いた名前を NFC にしたもの。ディスク上が NFD でも同じ。
    #[test]
    fn list_tree_title_is_nfc_file_name_without_extension(
        name in SAFE_NAME, store_as_nfd in any::<bool>()
    ) {
        let on_disk = if store_as_nfd { nfd(&name) } else { nfc(&name) };
        let v = VaultBuilder::new().file(&format!("{on_disk}.md"), "").open();
        let tree = v.list_tree().unwrap();
        prop_assert_eq!(tree.len(), 1);
        prop_assert_eq!(&tree[0].title, &nfc(&name));
    }

    /// list_tree が返した path をそのまま read_page に渡すと、必ず同じページに届く。
    #[test]
    fn path_from_list_tree_always_reaches_the_page(
        parent in SAFE_NAME, child in SAFE_NAME, store_as_nfd in any::<bool>()
    ) {
        let norm = |s: &str| if store_as_nfd { nfd(s) } else { nfc(s) };
        let v = VaultBuilder::new()
            .file(&format!("{}/{}.md", norm(&parent), norm(&child)), "子の中身")
            .open();
        let tree = v.list_tree().unwrap();
        let child_path = &tree[0].children[0].path;
        prop_assert_eq!(v.read_page(child_path).unwrap().0, "子の中身");
    }
}
