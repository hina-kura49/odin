//! 段階5の性質テスト(proptest)

mod common;

use common::*;
use proptest::prelude::*;
use unicode_segmentation::UnicodeSegmentation;

/// 本文の候補(空白は含めない。英字は大文字小文字を混ぜる。NFKC で形が変わる文字は含めない)。
const BODY: &str = "[ぁ-んァ-ン一-龠a-zA-Z0-9ー。、🍣]{1,300}";

fn graphemes(s: &str) -> usize {
    s.graphemes(true).count()
}

/// 本文から、文字の位置 `start` と長さ `len` で部分文字列を取り出す。
fn substring(body: &str, start: prop::sample::Index, len: usize) -> String {
    let chars: Vec<char> = body.chars().collect();
    let len = len.clamp(1, chars.len());
    let start = start.index(chars.len() - len + 1);
    chars[start..start + len].iter().collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// 本文のどの部分文字列で探しても、そのページが見つかる。英字は大文字小文字を問わない。
    #[test]
    fn any_substring_of_body_finds_the_page(
        body in BODY, start in any::<prop::sample::Index>(), len in 1usize..10, upper in any::<bool>()
    ) {
        let sub = substring(&body, start, len);
        let query = if upper { sub.to_uppercase() } else { sub.to_lowercase() };
        let v = VaultBuilder::new().file("p/a.md", &body).file("b.md", "無関係").open();

        let hits: Vec<String> = v.search(&query, 1000).unwrap().into_iter().map(|h| h.path).collect();

        prop_assert!(hits.contains(&"p/a.md".to_string()), "{query:?} で {body:?} が見つからない: {hits:?}");
    }

    /// 本文に一致したときの snippet は、決まり(回答51)を守る。
    #[test]
    fn snippet_follows_the_rules(body in BODY, start in any::<prop::sample::Index>(), len in 1usize..10) {
        let query = substring(&body, start, len);
        let v = VaultBuilder::new().file("a.md", &body).open();

        let hits = v.search(&query, 10).unwrap();
        prop_assert_eq!(hits.len(), 1);
        let s = &hits[0].snippet;

        // hit は本文で最初に現れる一致(大文字小文字だけが違ってよい)。
        let first = body.to_lowercase().find(&query.to_lowercase()).unwrap();
        prop_assert_eq!(s.hit.to_lowercase(), query.to_lowercase());
        let before_text = &body[..first];
        // before は直前の最大30文字。切り詰めたときだけ先頭に「…」。
        let before = s.before.strip_prefix('…').unwrap_or(&s.before);
        prop_assert_eq!(s.before.starts_with('…'), graphemes(before_text) > 30);
        prop_assert!(before_text.ends_with(before));
        prop_assert_eq!(graphemes(before), graphemes(before_text).min(30));
        // 全体は最大120文字。after は本文の続きで、切り詰めたときだけ末尾に「…」。
        let after = s.after.strip_suffix('…').unwrap_or(&s.after);
        let after_text = &body[first + s.hit.len()..];
        prop_assert!(after_text.starts_with(after));
        prop_assert!(graphemes(before) + graphemes(&s.hit) + graphemes(after) <= 120);
        prop_assert_eq!(s.after.ends_with('…'), graphemes(after) < graphemes(after_text));
    }

    /// 空白で区切った語は、すべてを含むページだけが見つかる(順序は問わない)。
    #[test]
    fn several_words_find_pages_containing_all_of_them(
        bodies in prop::collection::vec(BODY, 1..6), w1 in "[ぁ-んa-z]{1,2}", w2 in "[ぁ-んa-z]{1,2}"
    ) {
        let mut b = VaultBuilder::new();
        for (i, body) in bodies.iter().enumerate() {
            b = b.file(&format!("p{i}.md"), body);
        }
        let v = b.open();

        let found: std::collections::BTreeSet<String> =
            v.search(&format!("{w2} {w1}"), 1000).unwrap().into_iter().map(|h| h.path).collect();

        let contains = |hay: &str, w: &str| hay.to_lowercase().contains(&w.to_lowercase());
        let expected: std::collections::BTreeSet<String> = bodies
            .iter()
            .enumerate()
            .map(|(i, body)| (format!("p{i}"), body))
            .filter(|(title, body)| {
                let page = format!("{title}\n{body}");
                contains(&page, &w1) && contains(&page, &w2)
            })
            .map(|(title, _)| format!("{title}.md"))
            .collect();
        prop_assert_eq!(found, expected);
    }

    /// 索引を消して開き直しても、search と recent_pages の結果は変わらない。
    #[test]
    fn reopening_without_index_gives_same_results(
        bodies in prop::collection::vec(BODY, 1..6), query in "[ぁ-んa-z]{1,2}"
    ) {
        let mut b = VaultBuilder::new();
        for (i, body) in bodies.iter().enumerate() {
            b = b.file(&format!("d{}/p{i}.md", i % 2), body);
        }
        let mut v = b.open();
        let observe = |v: &odin_core::Vault| {
            (v.search(&query, 100).unwrap(), v.search(&bodies[0], 100).unwrap(), v.recent_pages(100).unwrap())
        };
        let before = observe(&v);

        v.delete_index_and_reopen();

        prop_assert_eq!(observe(&v), before);
    }
}
