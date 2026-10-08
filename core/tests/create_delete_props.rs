//! 段階3の性質テスト(proptest)

mod common;

use common::*;
use proptest::prelude::*;

/// タイトルの候補。名前に使えない文字(/ : \ 制御文字)を混ぜる。
/// 前後の空白の除去・先頭の「.」・長さの切り詰めは個別のテストで確かめるので、
/// ここでは端に空白類や「.」を置かず、長さも 200 バイト未満にする。
const TITLE: &str = "[a-zA-Zあ-んア-ン一-龠0-9０-９📝/:\\\\\u{0}\u{7F}]([ a-zA-Zあ-んア-ン一-龠0-9０-９📝/:\\\\\u{0}\n\t\u{7F}\u{85}.]{0,14}[a-zA-Zあ-んア-ン一-龠0-9０-９📝/:\\\\\u{0}\u{7F}])?";

fn is_forbidden(c: char) -> bool {
    c == '/' || c == ':' || c == '\\' || c.is_control()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// どんなタイトルでも、使えない文字だけが「-」に置き換わり、ほかの文字は変わらない。
    #[test]
    fn created_title_replaces_only_forbidden_characters(title in TITLE) {
        let v = VaultBuilder::new().open();

        let meta = v.create_page(None, &title).unwrap();

        let expected: String = title.chars().map(|c| if is_forbidden(c) { '-' } else { c }).collect();
        prop_assert_eq!(&meta.title, &nfc(&expected));
        prop_assert_eq!(&meta.path, &format!("{}.md", meta.title));
    }

    /// 作ったページは、返った path で読めて、list_tree にも出る。
    #[test]
    fn created_page_is_reachable(title in TITLE) {
        let v = VaultBuilder::new().file("p.md", "").open();

        let meta = v.create_page(Some("p.md"), &title).unwrap();

        prop_assert!(v.read_page(&meta.path).is_ok());
        prop_assert!(find(&v.list_tree().unwrap(), &meta.path).is_some());
    }

    /// 同じタイトルで何度作っても、すべて別のページになり、既存のページを上書きしない。
    #[test]
    fn creating_same_title_repeatedly_makes_distinct_pages(title in TITLE, n in 2usize..5) {
        let v = VaultBuilder::new().open();

        let paths: std::collections::BTreeSet<String> =
            (0..n).map(|_| v.create_page(None, &title).unwrap().path).collect();

        prop_assert_eq!(paths.len(), n);
        prop_assert_eq!(all_paths(&v.list_tree().unwrap()), paths);
    }

    /// 作ってから消すと、vault は作る前と同じ状態に戻る。
    /// (子のない親の下に作った場合も、空になったフォルダが消えるので元に戻る。)
    #[test]
    fn create_then_delete_restores_vault(title in TITLE, parent in prop::sample::select(vec![None, Some("p.md"), Some("r.md"), Some("f")])) {
        let v = VaultBuilder::new().file("p.md", "").file("p/q.md", "").file("r.md", "").file("f/g.md", "").open();
        let before = v.all_entries();

        let meta = v.create_page(parent, &title).unwrap();
        v.delete_page(&meta.path).unwrap();

        prop_assert_eq!(v.all_entries(), before);
    }
}
