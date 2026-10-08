//! 段階5の性質テスト(proptest)

mod common;

use common::*;
use proptest::prelude::*;

/// 本文の候補(空白は含めない。英字は大文字小文字を混ぜる)。
const BODY: &str = "[ぁ-んァ-ン一-龠a-zA-Z0-9ー。、🍣]{2,40}";

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// 本文のどの部分文字列(2文字以上)で探しても、そのページが見つかる。英字は大文字小文字を問わない。
    #[test]
    fn any_substring_of_body_finds_the_page(
        body in BODY, start in any::<prop::sample::Index>(), len in 2usize..10, upper in any::<bool>()
    ) {
        let chars: Vec<char> = body.chars().collect();
        let len = len.min(chars.len());
        let start = start.index(chars.len() - len + 1);
        let sub: String = chars[start..start + len].iter().collect();
        let query = if upper { sub.to_uppercase() } else { sub.to_lowercase() };
        let v = VaultBuilder::new().file("p/a.md", &body).file("b.md", "無関係").open();

        let hits: Vec<String> = v.search(&query).unwrap().into_iter().map(|h| h.path).collect();

        prop_assert!(hits.contains(&"p/a.md".to_string()), "{query:?} で {body:?} が見つからない: {hits:?}");
    }

    /// 索引を消して作り直しても、search と recent_pages の結果は変わらない。
    #[test]
    fn rebuilding_from_scratch_gives_same_results(
        bodies in prop::collection::vec(BODY, 1..6), query in "[ぁ-んa-z]{2}"
    ) {
        let mut b = VaultBuilder::new();
        for (i, body) in bodies.iter().enumerate() {
            b = b.file(&format!("d{}/p{i}.md", i % 2), body);
        }
        let mut v = b.open();
        let before = (v.search(&query).unwrap(), v.search(&bodies[0]).unwrap(), v.recent_pages(100).unwrap());

        v.delete_index_and_reopen();
        v.rebuild_index().unwrap();

        let after = (v.search(&query).unwrap(), v.search(&bodies[0]).unwrap(), v.recent_pages(100).unwrap());
        prop_assert_eq!(after, before);
    }
}
