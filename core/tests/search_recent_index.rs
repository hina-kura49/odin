//! 段階5: search / recent_pages / rescan / rebuild_index

mod common;

use std::collections::BTreeSet;
use std::time::{Duration, SystemTime};

use common::*;
use odin_core::{PageMeta, SearchHit, Snippet, Vault};

fn t(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

fn search(v: &Vault, query: &str) -> Vec<SearchHit> {
    v.search(query, 1000).unwrap()
}

fn hit_paths(v: &Vault, query: &str) -> Vec<String> {
    search(v, query).into_iter().map(|h| h.path).collect()
}

fn hit_set(v: &Vault, query: &str) -> BTreeSet<String> {
    hit_paths(v, query).into_iter().collect()
}

fn snippet(v: &Vault, query: &str) -> Snippet {
    let hits = search(v, query);
    assert_eq!(hits.len(), 1, "{query:?}: {hits:?}");
    hits.into_iter().next().unwrap().snippet
}

fn sn(before: &str, hit: &str, after: &str) -> Snippet {
    Snippet {
        before: before.into(),
        hit: hit.into(),
        after: after.into(),
    }
}

fn recent_paths(v: &Vault, limit: usize) -> Vec<String> {
    v.recent_pages(limit)
        .unwrap()
        .into_iter()
        .map(|m| m.path)
        .collect()
}

fn set(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|p| p.to_string()).collect()
}

// ================================================================ search: 基本

#[test]
fn search_finds_page_by_word_in_body() {
    let v = VaultBuilder::new()
        .file("a.md", "今日は会議があった")
        .file("b.md", "関係ない")
        .open();

    assert_eq!(hit_paths(&v, "会議"), ["a.md"]);
}

#[test]
fn search_finds_page_by_word_in_title() {
    let v = VaultBuilder::new()
        .file("週次会議.md", "")
        .file("b.md", "関係ない")
        .open();

    assert_eq!(hit_paths(&v, "会議"), ["週次会議.md"]);
}

#[test]
fn search_returns_page_title_in_hit() {
    let v = VaultBuilder::new().file("p/週次会議.md", "会議").open();

    let hits = search(&v, "会議");

    assert_eq!(hits[0].path, "p/週次会議.md");
    assert_eq!(hits[0].title, "週次会議");
}

#[test]
fn search_returns_nothing_when_no_page_matches() {
    let v = VaultBuilder::new().file("a.md", "本文").open();
    assert!(search(&v, "見つからない語").is_empty());
}

#[test]
fn search_returns_each_page_once_even_if_title_and_body_both_match() {
    let v = VaultBuilder::new().file("会議.md", "会議 会議 会議").open();
    assert_eq!(hit_paths(&v, "会議"), ["会議.md"]);
}

#[test]
fn search_returns_nfc_path_and_title_for_nfd_named_page() {
    let v = VaultBuilder::new()
        .file(&nfd("がっこう.md"), "時間割")
        .open();

    let hits = search(&v, "時間割");

    assert_eq!(hits[0].path, nfc("がっこう.md"));
    assert_eq!(hits[0].title, nfc("がっこう"));
}

#[test]
fn search_finds_nested_pages() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b/c.md", "深いところの単語")
        .open();

    assert_eq!(hit_paths(&v, "深いところ"), ["a/b/c.md"]);
}

#[test]
fn search_returns_every_matching_page() {
    let v = VaultBuilder::new()
        .file("a.md", "りんご")
        .file("b.md", "りんごとみかん")
        .file("c/d.md", "青りんご")
        .file("e.md", "みかん")
        .open();

    assert_eq!(hit_set(&v, "りんご"), set(&["a.md", "b.md", "c/d.md"]));
}

// ================================================================ search: 空の検索語(回答45)

#[test]
fn search_with_empty_query_returns_nothing() {
    let v = VaultBuilder::new().file("a.md", "本文").open();
    assert!(search(&v, "").is_empty());
}

#[test]
fn search_with_whitespace_only_query_returns_nothing() {
    let v = VaultBuilder::new().file("a.md", "本文 \u{3000}").open();
    assert!(search(&v, " \u{3000}\t").is_empty());
}

// ================================================================ search: 日本語・英字・表記の違い(回答46〜48)

#[test]
fn search_finds_japanese_substring_in_the_middle_of_a_word() {
    let v = VaultBuilder::new()
        .file("a.md", "日本語の検索エンジン")
        .open();
    assert_eq!(hit_paths(&v, "本語の検"), ["a.md"]);
}

#[test]
fn search_finds_japanese_word_at_start_of_longer_word() {
    let v = VaultBuilder::new()
        .file("a.md", "京都タワーに行った")
        .open();
    assert_eq!(hit_paths(&v, "京都"), ["a.md"]);
}

#[test]
fn search_does_not_fail_on_kyoto_inside_tokyo_to() {
    // 「東京都」が「京都」で見つかるかどうかは、どちらでもよい(仕様8)。エラーにならないことだけ確かめる。
    let v = VaultBuilder::new()
        .file("a.md", "東京都に住んでいる")
        .open();
    assert!(v.search("京都", 10).is_ok());
}

#[test]
fn search_finds_page_by_single_character() {
    let v = VaultBuilder::new()
        .file("a.md", "黒猫")
        .file("b.md", "犬")
        .open();

    assert_eq!(hit_paths(&v, "猫"), ["a.md"]);
}

#[test]
fn search_finds_single_ascii_letter() {
    let v = VaultBuilder::new()
        .file("a.md", "xyz")
        .file("b.md", "abc")
        .open();
    assert_eq!(hit_paths(&v, "Y"), ["a.md"]);
}

#[test]
fn search_ignores_letter_case_of_query() {
    let v = VaultBuilder::new().file("a.md", "Rust で書く").open();
    assert_eq!(hit_paths(&v, "rust"), ["a.md"]);
    assert_eq!(hit_paths(&v, "RUST"), ["a.md"]);
}

#[test]
fn search_ignores_letter_case_of_body() {
    let v = VaultBuilder::new()
        .file("a.md", "rust")
        .file("b.md", "RUST")
        .file("c.md", "RuSt")
        .open();

    assert_eq!(hit_set(&v, "Rust"), set(&["a.md", "b.md", "c.md"]));
}

#[test]
fn search_ignores_letter_case_in_title() {
    let v = VaultBuilder::new().file("README.md", "").open();
    assert_eq!(hit_paths(&v, "readme"), ["README.md"]);
}

#[test]
fn search_finds_english_substring_inside_word() {
    let v = VaultBuilder::new().file("a.md", "documentation").open();
    assert_eq!(hit_paths(&v, "MENTAT"), ["a.md"]);
}

#[test]
fn search_finds_nfd_body_by_nfc_query() {
    let v = VaultBuilder::new().file("a.md", nfd("がっこう")).open();
    assert_eq!(hit_paths(&v, &nfc("がっこう")), ["a.md"]);
}

#[test]
fn search_finds_nfc_body_by_nfd_query() {
    let v = VaultBuilder::new().file("a.md", nfc("がっこう")).open();
    assert_eq!(hit_paths(&v, &nfd("がっこう")), ["a.md"]);
}

#[test]
fn search_finds_nfd_title_by_nfc_query() {
    let v = VaultBuilder::new().file(&nfd("ばんごう.md"), "").open();
    assert_eq!(hit_paths(&v, &nfc("ばんごう")), [nfc("ばんごう.md")]);
}

#[test]
fn search_treats_fullwidth_alphanumerics_as_halfwidth() {
    let v = VaultBuilder::new().file("a.md", "ＡＢＣ１２３").open();
    assert_eq!(hit_paths(&v, "abc123"), ["a.md"]);
}

#[test]
fn search_treats_halfwidth_query_as_fullwidth_body() {
    let v = VaultBuilder::new().file("a.md", "abc123").open();
    assert_eq!(hit_paths(&v, "ＡＢＣ１２３"), ["a.md"]);
}

#[test]
fn search_ignores_case_of_fullwidth_letters() {
    let v = VaultBuilder::new().file("a.md", "ａｂｃ").open();
    assert_eq!(hit_paths(&v, "ABC"), ["a.md"]);
}

#[test]
fn search_treats_fullwidth_symbols_as_halfwidth() {
    let v = VaultBuilder::new()
        .file("a.md", "型番 ＡＢＣ－１２３＃")
        .open();
    assert_eq!(hit_paths(&v, "abc-123#"), ["a.md"]);
}

#[test]
fn search_treats_halfwidth_katakana_as_fullwidth() {
    let v = VaultBuilder::new()
        .file("a.md", "ｶﾞｲﾄﾞ")
        .file("b.md", "ガイド")
        .open();

    assert_eq!(hit_set(&v, "ガイド"), set(&["a.md", "b.md"]));
    assert_eq!(hit_set(&v, "ｶﾞｲﾄﾞ"), set(&["a.md", "b.md"]));
}

#[test]
fn search_distinguishes_hiragana_from_katakana() {
    let v = VaultBuilder::new()
        .file("a.md", "カタカナ")
        .file("b.md", "かたかな")
        .open();

    assert_eq!(hit_paths(&v, "かたかな"), ["b.md"]);
    assert_eq!(hit_paths(&v, "カタカナ"), ["a.md"]);
}

#[test]
fn search_finds_emoji() {
    let v = VaultBuilder::new()
        .file("a.md", "今日の寿司🍣はおいしい")
        .open();
    assert_eq!(hit_paths(&v, "寿司🍣"), ["a.md"]);
}

#[test]
fn search_finds_text_in_large_file() {
    let body = format!("{}最後の行にある目印\n", "あいうえお\n".repeat(400_000));
    let v = VaultBuilder::new().file("big.md", body).open();

    assert_eq!(hit_paths(&v, "最後の行にある目印"), ["big.md"]);
}

#[test]
fn search_finds_text_in_crlf_and_bom_page() {
    let v = VaultBuilder::new()
        .file("a.md", "\u{FEFF}1行目\r\n検索語\r\n")
        .open();
    assert_eq!(hit_paths(&v, "検索語"), ["a.md"]);
}

// ================================================================ search: 複数の語(回答49)

#[test]
fn search_with_several_words_finds_pages_containing_all_of_them() {
    let v = VaultBuilder::new()
        .file("both.md", "定例の会議の議事録")
        .file("one.md", "会議だけ")
        .file("other.md", "議事録だけ")
        .open();

    assert_eq!(hit_paths(&v, "会議 議事録"), ["both.md"]);
}

#[test]
fn search_with_several_words_ignores_their_order_and_position() {
    let v = VaultBuilder::new()
        .file("a.md", "議事録は最後に。最初に会議")
        .open();

    assert_eq!(hit_paths(&v, "会議 議事録"), ["a.md"]);
}

#[test]
fn search_splits_words_by_fullwidth_space_and_repeated_spaces() {
    let v = VaultBuilder::new()
        .file("a.md", "会議と議事録")
        .file("b.md", "会議")
        .open();

    assert_eq!(hit_paths(&v, "会議\u{3000}議事録"), ["a.md"]);
    assert_eq!(hit_paths(&v, "  会議 \u{3000}\t 議事録  "), ["a.md"]);
}

#[test]
fn search_treats_symbols_literally() {
    let v = VaultBuilder::new()
        .file("a.md", "C++ 入門")
        .file("b.md", "C 入門")
        .open();

    assert_eq!(hit_paths(&v, "C++"), ["a.md"]);
}

#[test]
fn search_treats_quotes_literally() {
    let v = VaultBuilder::new()
        .file("a.md", "「\"会議\"」と書いた")
        .file("b.md", "会議")
        .open();

    assert_eq!(hit_paths(&v, "\"会議\""), ["a.md"]);
}

#[test]
fn search_treats_wildcard_like_characters_literally() {
    let v = VaultBuilder::new()
        .file("a.md", "100% 達成")
        .file("b.md", "100 達成")
        .file("c.md", "a_b と a*b")
        .file("d.md", "axb")
        .open();

    assert_eq!(hit_paths(&v, "100%"), ["a.md"]);
    assert_eq!(hit_paths(&v, "a_b"), ["c.md"]);
    assert_eq!(hit_paths(&v, "a*b"), ["c.md"]);
}

// ================================================================ search: ファイルの全文が対象(回答50)

#[test]
fn search_finds_text_in_frontmatter() {
    let v = VaultBuilder::new()
        .file("a.md", "---\ntags: [秘密のタグ]\n---\n本文")
        .open();

    assert_eq!(hit_paths(&v, "秘密のタグ"), ["a.md"]);
}

#[test]
fn search_finds_link_destination() {
    let v = VaultBuilder::new()
        .file("a.md", "[表示](隠れた行き先.md)")
        .open();
    assert_eq!(hit_paths(&v, "隠れた行き先"), ["a.md"]);
}

#[test]
fn search_finds_text_in_code_block() {
    let v = VaultBuilder::new()
        .file("a.md", "```rust\nfn odin_main() {}\n```\n")
        .open();
    assert_eq!(hit_paths(&v, "odin_main"), ["a.md"]);
}

#[test]
fn search_finds_markdown_symbols() {
    let v = VaultBuilder::new()
        .file("a.md", "これは **太字** です")
        .file("b.md", "これは 太字 です")
        .open();

    assert_eq!(hit_paths(&v, "**太"), ["a.md"]);
}

// ================================================================ search: 並び順(仕様8・回答52)

#[test]
fn search_ranks_title_match_above_body_match() {
    let v = VaultBuilder::new()
        .file("メモ.md", "定例の会議について")
        .file("会議.md", "")
        .mtime("メモ.md", t(1_700_000_000))
        .mtime("会議.md", t(1_600_000_000))
        .open();

    assert_eq!(hit_paths(&v, "会議"), ["会議.md", "メモ.md"]);
}

#[test]
fn search_ranks_every_title_match_above_every_body_match() {
    let v = VaultBuilder::new()
        .file("b1.md", "会議 会議 会議 会議 会議")
        .file("b2.md", "会議")
        .file("会議録.md", "")
        .file("x/定例会議.md", "")
        .mtime("b1.md", t(1_900_000_000))
        .mtime("b2.md", t(1_800_000_000))
        .mtime("会議録.md", t(1_600_000_000))
        .mtime("x/定例会議.md", t(1_500_000_000))
        .open();

    assert_eq!(
        hit_paths(&v, "会議"),
        ["会議録.md", "x/定例会議.md", "b1.md", "b2.md"]
    );
}

#[test]
fn search_ranks_title_match_ignoring_case_above_body_match() {
    let v = VaultBuilder::new()
        .file("notes.md", "Rust のメモ")
        .file("RUST入門.md", "")
        .open();

    assert_eq!(hit_paths(&v, "rust"), ["RUST入門.md", "notes.md"]);
}

#[test]
fn search_orders_body_matches_newest_first() {
    let v = VaultBuilder::new()
        .file("old.md", "合言葉")
        .file("new.md", "合言葉")
        .file("p/mid.md", "合言葉")
        .mtime("old.md", t(1_600_000_000))
        .mtime("p/mid.md", t(1_650_000_000))
        .mtime("new.md", t(1_700_000_000))
        .open();

    assert_eq!(hit_paths(&v, "合言葉"), ["new.md", "p/mid.md", "old.md"]);
}

#[test]
fn search_orders_title_matches_newest_first() {
    let v = VaultBuilder::new()
        .file("合言葉1.md", "")
        .file("合言葉2.md", "")
        .mtime("合言葉1.md", t(1_700_000_000))
        .mtime("合言葉2.md", t(1_600_000_000))
        .open();

    assert_eq!(hit_paths(&v, "合言葉"), ["合言葉1.md", "合言葉2.md"]);
}

#[test]
fn search_orders_ties_by_natural_path_order() {
    let v = VaultBuilder::new()
        .file("p10.md", "合言葉")
        .file("P2.md", "合言葉")
        .file("p1.md", "合言葉")
        .mtime("p10.md", t(1_600_000_000))
        .mtime("P2.md", t(1_600_000_000))
        .mtime("p1.md", t(1_600_000_000))
        .open();

    assert_eq!(hit_paths(&v, "合言葉"), ["p1.md", "P2.md", "p10.md"]);
}

// ================================================================ search: limit(回答53)

#[test]
fn search_returns_top_results_up_to_limit() {
    let v = VaultBuilder::new()
        .file("a.md", "合言葉")
        .file("b.md", "合言葉")
        .file("c.md", "合言葉")
        .file("合言葉.md", "")
        .mtime("a.md", t(1_600_000_000))
        .mtime("b.md", t(1_700_000_000))
        .mtime("c.md", t(1_800_000_000))
        .open();

    let paths: Vec<String> = v
        .search("合言葉", 2)
        .unwrap()
        .into_iter()
        .map(|h| h.path)
        .collect();

    assert_eq!(paths, ["合言葉.md", "c.md"]);
}

#[test]
fn search_with_zero_limit_returns_nothing() {
    let v = VaultBuilder::new().file("a.md", "合言葉").open();
    assert!(v.search("合言葉", 0).unwrap().is_empty());
}

#[test]
fn search_with_limit_larger_than_matches_returns_all() {
    let v = VaultBuilder::new()
        .file("a.md", "合言葉")
        .file("b.md", "合言葉")
        .open();
    assert_eq!(v.search("合言葉", 100).unwrap().len(), 2);
}

// ================================================================ search: snippet(回答51)

#[test]
fn snippet_has_whole_text_when_body_is_short() {
    let v = VaultBuilder::new().file("a.md", "前置き目印後ろ").open();
    assert_eq!(snippet(&v, "目印"), sn("前置き", "目印", "後ろ"));
}

#[test]
fn snippet_has_empty_before_when_match_is_at_start() {
    let v = VaultBuilder::new().file("a.md", "目印のあと").open();
    assert_eq!(snippet(&v, "目印"), sn("", "目印", "のあと"));
}

#[test]
fn snippet_keeps_up_to_30_characters_before_without_ellipsis() {
    let v = VaultBuilder::new()
        .file("a.md", format!("{}目印", "あ".repeat(30)))
        .open();
    assert_eq!(snippet(&v, "目印"), sn(&"あ".repeat(30), "目印", ""));
}

#[test]
fn snippet_truncates_before_to_30_characters_with_leading_ellipsis() {
    let v = VaultBuilder::new()
        .file("a.md", format!("{}目印", "あ".repeat(31)))
        .open();
    assert_eq!(
        snippet(&v, "目印"),
        sn(&format!("…{}", "あ".repeat(30)), "目印", "")
    );
}

#[test]
fn snippet_fills_after_up_to_120_characters_in_total() {
    let v = VaultBuilder::new()
        .file("a.md", format!("目印{}", "い".repeat(118)))
        .open();
    assert_eq!(snippet(&v, "目印"), sn("", "目印", &"い".repeat(118)));
}

#[test]
fn snippet_truncates_after_with_trailing_ellipsis() {
    let v = VaultBuilder::new()
        .file("a.md", format!("目印{}", "い".repeat(119)))
        .open();
    assert_eq!(
        snippet(&v, "目印"),
        sn("", "目印", &format!("{}…", "い".repeat(118)))
    );
}

#[test]
fn snippet_truncates_both_sides() {
    let body = format!("{}目印{}", "あ".repeat(50), "い".repeat(100));
    let v = VaultBuilder::new().file("a.md", body).open();

    assert_eq!(
        snippet(&v, "目印"),
        sn(
            &format!("…{}", "あ".repeat(30)),
            "目印",
            &format!("{}…", "い".repeat(88))
        )
    );
}

#[test]
fn snippet_counts_graphemes_not_code_points() {
    let family = "👨\u{200D}👩\u{200D}👧";
    let v = VaultBuilder::new()
        .file(
            "a.md",
            format!("{}目印{}", family.repeat(40), family.repeat(100)),
        )
        .open();

    assert_eq!(
        snippet(&v, "目印"),
        sn(
            &format!("…{}", family.repeat(30)),
            "目印",
            &format!("{}…", family.repeat(88))
        )
    );
}

#[test]
fn snippet_collapses_newlines_and_runs_of_whitespace_into_one_space() {
    let v = VaultBuilder::new()
        .file(
            "a.md",
            "一行目\n\n  二行目\r\n目印\t\t三行目\u{3000}\u{3000}四行目",
        )
        .open();

    assert_eq!(
        snippet(&v, "目印"),
        sn("一行目 二行目 ", "目印", " 三行目 四行目")
    );
}

#[test]
fn snippet_hit_keeps_original_letter_case() {
    let v = VaultBuilder::new().file("a.md", "これは RUST です").open();
    assert_eq!(snippet(&v, "rust"), sn("これは ", "RUST", " です"));
}

#[test]
fn snippet_hit_keeps_original_width() {
    let v = VaultBuilder::new()
        .file("a.md", "型番ＡＢＣ－１２３です")
        .open();
    assert_eq!(snippet(&v, "abc-123"), sn("型番", "ＡＢＣ－１２３", "です"));
}

#[test]
fn snippet_hit_keeps_original_normalization() {
    let body = format!("私の{}です", nfd("がっこう"));
    let v = VaultBuilder::new().file("a.md", &body).open();

    assert_eq!(
        snippet(&v, &nfc("がっこう")),
        sn("私の", &nfd("がっこう"), "です")
    );
}

#[test]
fn snippet_uses_first_occurrence() {
    let v = VaultBuilder::new()
        .file("a.md", "前に目印、後にも目印")
        .open();
    assert_eq!(snippet(&v, "目印"), sn("前に", "目印", "、後にも目印"));
}

#[test]
fn snippet_uses_earliest_occurrence_among_several_words() {
    let v = VaultBuilder::new()
        .file("a.md", "前の会議と後の議事録")
        .open();
    assert_eq!(
        snippet(&v, "議事録 会議"),
        sn("前の", "会議", "と後の議事録")
    );
}

#[test]
fn snippet_of_title_only_match_has_start_of_body_in_after() {
    let v = VaultBuilder::new().file("会議.md", "本文の\n先頭").open();
    assert_eq!(snippet(&v, "会議"), sn("", "", "本文の 先頭"));
}

#[test]
fn snippet_of_title_only_match_truncates_body_to_120_characters() {
    let v = VaultBuilder::new().file("会議.md", "う".repeat(130)).open();
    assert_eq!(
        snippet(&v, "会議"),
        sn("", "", &format!("{}…", "う".repeat(120)))
    );
}

#[test]
fn snippet_of_title_only_match_with_empty_body_is_empty() {
    let v = VaultBuilder::new().file("会議.md", "").open();
    assert_eq!(snippet(&v, "会議"), Snippet::default());
}

#[test]
fn snippet_shows_match_near_the_end_of_a_large_file() {
    let body = format!("{}最後の目印です", "あいうえお".repeat(400_000));
    let v = VaultBuilder::new().file("big.md", body).open();

    assert_eq!(
        snippet(&v, "最後の目印"),
        sn(
            &format!("…{}", "あいうえお".repeat(6)),
            "最後の目印",
            "です"
        )
    );
}

// ================================================================ search: 対象外

#[test]
fn search_excludes_hidden_files_and_hidden_folders() {
    let v = VaultBuilder::new()
        .file("a.md", "合言葉")
        .file(".hidden.md", "合言葉")
        .file(".obsidian/x.md", "合言葉")
        .file("p/.secret.md", "合言葉")
        .open();

    assert_eq!(hit_paths(&v, "合言葉"), ["a.md"]);
}

#[test]
fn search_excludes_non_markdown_files() {
    let v = VaultBuilder::new()
        .file("a.md", "合言葉")
        .file("notes.txt", "合言葉")
        .file("b.markdown", "合言葉")
        .file("合言葉.png", "png")
        .open();

    assert_eq!(hit_paths(&v, "合言葉"), ["a.md"]);
}

#[test]
fn search_excludes_symlinks() {
    let b = VaultBuilder::new()
        .file("a.md", "合言葉")
        .outside_file("outside.md", "合言葉");
    let inside = b.root().join("a.md");
    let outside = b.outside("outside.md");
    let v = b
        .symlink("in.md", &inside)
        .symlink("out.md", &outside)
        .open();

    assert_eq!(hit_paths(&v, "合言葉"), ["a.md"]);
}

#[test]
fn search_excludes_leftover_temp_files() {
    let v = VaultBuilder::new().file("a.md", "合言葉").open();
    v.write_outside_app(&format!("{}1", odin_core::TEMP_FILE_PREFIX), "合言葉");
    v.rescan().unwrap();

    assert_eq!(hit_paths(&v, "合言葉"), ["a.md"]);
}

#[test]
fn search_excludes_non_utf8_pages() {
    let mut sjis = vec![0x82, 0xA0];
    sjis.extend_from_slice(b" password");
    let v = VaultBuilder::new()
        .file("a.md", "password")
        .file("sjis.md", sjis)
        .open();

    assert_eq!(hit_paths(&v, "password"), ["a.md"]);
}

#[test]
fn search_excludes_files_whose_names_have_control_characters() {
    let v = VaultBuilder::new()
        .file("a.md", "合言葉")
        .file("tab\t.md", "合言葉")
        .open();

    assert_eq!(hit_paths(&v, "合言葉"), ["a.md"]);
}

#[test]
fn search_adds_nothing_to_vault() {
    let v = VaultBuilder::new()
        .file("a.md", "合言葉")
        .file("p/b.md", "")
        .open();
    let before = v.all_entries();

    search(&v, "合言葉");

    assert_eq!(v.all_entries(), before);
}

// ================================================================ search: アプリでの操作の即時反映

#[test]
fn search_reflects_write_page() {
    let v = VaultBuilder::new().file("a.md", "古い言葉").open();
    let (_, version) = v.read_page("a.md").unwrap();

    v.write_page("a.md", "新しい言葉", &version).unwrap();

    assert!(search(&v, "古い言葉").is_empty());
    assert_eq!(hit_paths(&v, "新しい言葉"), ["a.md"]);
}

#[test]
fn search_reflects_create_page() {
    let v = VaultBuilder::new().open();

    v.create_page(None, "新規ページ").unwrap();

    assert_eq!(hit_paths(&v, "新規ページ"), ["新規ページ.md"]);
}

#[test]
fn search_reflects_rename_page() {
    let v = VaultBuilder::new()
        .file("旧名.md", "本文の目印")
        .file("旧名/子.md", "子の目印")
        .open();

    v.rename_page("旧名.md", "新名").unwrap();

    assert!(search(&v, "旧名").is_empty());
    assert_eq!(hit_paths(&v, "新名"), ["新名.md"]);
    assert_eq!(hit_paths(&v, "子の目印"), ["新名/子.md"]);
}

#[test]
fn search_reflects_links_rewritten_by_rename() {
    let v = VaultBuilder::new()
        .file("旧名.md", "")
        .file("other.md", "[こちら](旧名.md)")
        .open();

    v.rename_page("旧名.md", "新名").unwrap();

    assert!(search(&v, "旧名").is_empty());
    assert_eq!(hit_set(&v, "新名"), set(&["新名.md", "other.md"]));
}

#[test]
fn search_reflects_delete_page() {
    let v = VaultBuilder::new()
        .file("a.md", "合言葉")
        .file("a/b.md", "合言葉")
        .file("c.md", "合言葉")
        .open();

    v.delete_page("a.md").unwrap();

    assert_eq!(hit_paths(&v, "合言葉"), ["c.md"]);
}

// ================================================================ アプリの外での変更: rescan(回答54)

#[test]
fn rescan_reflects_page_added_outside_the_app() {
    let v = VaultBuilder::new().file("a.md", "").open();
    search(&v, "外で追加");
    v.write_outside_app("p/new.md", "外で追加");

    v.rescan().unwrap();

    assert_eq!(hit_paths(&v, "外で追加"), ["p/new.md"]);
    assert!(recent_paths(&v, 10).contains(&"p/new.md".to_string()));
    assert!(find(&v.list_tree().unwrap(), "p/new.md").is_some());
}

#[test]
fn rescan_reflects_page_changed_outside_the_app() {
    let v = VaultBuilder::new()
        .file("a.md", "変更前")
        .mtime("a.md", t(1_600_000_000))
        .open();
    search(&v, "変更前");
    v.write_outside_app("a.md", "中身を変えた後");
    v.set_mtime("a.md", t(1_700_000_000));

    v.rescan().unwrap();

    assert!(search(&v, "変更前").is_empty());
    assert_eq!(hit_paths(&v, "変えた後"), ["a.md"]);
    assert_eq!(v.recent_pages(1).unwrap()[0].modified_at, 1_700_000_000_000);
}

#[test]
fn rescan_reflects_page_deleted_outside_the_app() {
    let v = VaultBuilder::new()
        .file("a.md", "合言葉")
        .file("b.md", "合言葉")
        .open();
    search(&v, "合言葉");
    std::fs::remove_file(v.root().join("a.md")).unwrap();

    v.rescan().unwrap();

    assert_eq!(hit_paths(&v, "合言葉"), ["b.md"]);
    assert_eq!(recent_paths(&v, 10), ["b.md"]);
    assert_eq!(all_paths(&v.list_tree().unwrap()), set(&["b.md"]));
}

#[test]
fn rescan_reflects_folder_renamed_outside_the_app() {
    let v = VaultBuilder::new().file("old/a.md", "合言葉").open();
    search(&v, "合言葉");
    std::fs::rename(v.root().join("old"), v.root().join("new")).unwrap();

    v.rescan().unwrap();

    assert_eq!(hit_paths(&v, "合言葉"), ["new/a.md"]);
}

#[test]
fn rescan_does_not_modify_vault() {
    let v = VaultBuilder::new()
        .file("a.md", "x")
        .file("p/b.md", "y")
        .open();
    v.write_outside_app("c.md", "z");
    let before: Vec<_> = ["a.md", "p/b.md", "c.md"]
        .iter()
        .map(|r| (v.disk_bytes(r), mtime(&v.root().join(r))))
        .collect();
    let entries = v.all_entries();

    v.rescan().unwrap();

    let after: Vec<_> = ["a.md", "p/b.md", "c.md"]
        .iter()
        .map(|r| (v.disk_bytes(r), mtime(&v.root().join(r))))
        .collect();
    assert_eq!(after, before);
    assert_eq!(v.all_entries(), entries);
}

#[test]
fn reopen_reflects_changes_made_outside_while_closed() {
    let mut v = VaultBuilder::new().file("a.md", "合言葉").open();
    search(&v, "合言葉");
    std::fs::remove_file(v.root().join("a.md")).unwrap();
    v.write_outside_app("b.md", "合言葉");

    v.reopen();

    assert_eq!(hit_paths(&v, "合言葉"), ["b.md"]);
}

#[test]
fn rebuild_index_reflects_change_with_same_size_and_mtime() {
    // rescan は見逃してよいが、rebuild_index はファイルから作り直すので必ず反映する。
    let v = VaultBuilder::new()
        .file("a.md", "内容A")
        .mtime("a.md", t(1_600_000_000))
        .open();
    search(&v, "内容A");
    v.write_outside_app("a.md", "内容B");
    v.set_mtime("a.md", t(1_600_000_000));

    v.rebuild_index().unwrap();

    assert!(search(&v, "内容A").is_empty());
    assert_eq!(hit_paths(&v, "内容B"), ["a.md"]);
}

#[test]
fn rebuild_index_reflects_outside_changes() {
    let v = VaultBuilder::new()
        .file("a.md", "合言葉")
        .file("b.md", "合言葉")
        .open();
    search(&v, "合言葉");
    std::fs::remove_file(v.root().join("a.md")).unwrap();
    v.write_outside_app("c.md", "合言葉");

    v.rebuild_index().unwrap();

    assert_eq!(hit_set(&v, "合言葉"), set(&["b.md", "c.md"]));
}

// ================================================================ recent_pages

#[test]
fn recent_pages_returns_pages_newest_first() {
    let v = VaultBuilder::new()
        .file("old.md", "")
        .file("new.md", "")
        .file("p/mid.md", "")
        .mtime("old.md", t(1_600_000_000))
        .mtime("p/mid.md", t(1_650_000_000))
        .mtime("new.md", t(1_700_000_000))
        .open();

    assert_eq!(recent_paths(&v, 10), ["new.md", "p/mid.md", "old.md"]);
}

#[test]
fn recent_pages_orders_ties_by_natural_path_order() {
    let v = VaultBuilder::new()
        .file("p10.md", "")
        .file("P2.md", "")
        .file("p1.md", "")
        .mtime("p10.md", t(1_600_000_000))
        .mtime("P2.md", t(1_600_000_000))
        .mtime("p1.md", t(1_600_000_000))
        .open();

    assert_eq!(recent_paths(&v, 10), ["p1.md", "P2.md", "p10.md"]);
}

#[test]
fn recent_pages_returns_at_most_limit_pages() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("b.md", "")
        .file("c.md", "")
        .mtime("a.md", t(1_600_000_000))
        .mtime("b.md", t(1_700_000_000))
        .mtime("c.md", t(1_800_000_000))
        .open();

    assert_eq!(recent_paths(&v, 2), ["c.md", "b.md"]);
}

#[test]
fn recent_pages_with_zero_limit_is_empty() {
    let v = VaultBuilder::new().file("a.md", "").open();
    assert!(v.recent_pages(0).unwrap().is_empty());
}

#[test]
fn recent_pages_of_empty_vault_is_empty() {
    let v = VaultBuilder::new().open();
    assert!(v.recent_pages(10).unwrap().is_empty());
}

#[test]
fn recent_pages_returns_modified_at_as_mtime_in_milliseconds() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .mtime("a.md", t(1_600_000_000) + Duration::from_millis(123))
        .open();

    let pages = v.recent_pages(10).unwrap();

    assert_eq!(
        pages,
        [PageMeta {
            path: "a.md".into(),
            title: "a".into(),
            modified_at: 1_600_000_000_123
        }]
    );
}

#[test]
fn recent_pages_returns_nfc_path_and_title() {
    let v = VaultBuilder::new().file(&nfd("がっこう.md"), "").open();

    let pages = v.recent_pages(10).unwrap();

    assert_eq!(pages[0].path, nfc("がっこう.md"));
    assert_eq!(pages[0].title, nfc("がっこう"));
}

#[test]
fn recent_pages_excludes_non_pages() {
    let b = VaultBuilder::new()
        .file("a.md", "")
        .file(".hidden.md", "")
        .file(".obsidian/x.md", "")
        .file("notes.txt", "")
        .file("folder/only.png", "png");
    let target = b.root().join("a.md");
    let v = b.symlink("link.md", &target).open();
    v.write_outside_app(&format!("{}1", odin_core::TEMP_FILE_PREFIX), "");
    v.rescan().unwrap();

    assert_eq!(recent_paths(&v, 100), ["a.md"]);
}

#[test]
fn recent_pages_puts_page_written_by_app_first() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("b.md", "")
        .mtime("a.md", t(1_600_000_000))
        .mtime("b.md", t(1_700_000_000))
        .open();
    let (_, version) = v.read_page("a.md").unwrap();

    v.write_page("a.md", "更新", &version).unwrap();

    assert_eq!(recent_paths(&v, 10), ["a.md", "b.md"]);
}

#[test]
fn recent_pages_reflects_mtime_changed_outside_after_rescan() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("b.md", "")
        .mtime("a.md", t(1_700_000_000))
        .mtime("b.md", t(1_600_000_000))
        .open();
    v.recent_pages(10).unwrap();
    v.set_mtime("b.md", t(1_800_000_000));

    v.rescan().unwrap();

    assert_eq!(recent_paths(&v, 10), ["b.md", "a.md"]);
}

#[test]
fn recent_pages_reflects_delete_page() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .file("a/b.md", "")
        .file("c.md", "")
        .open();

    v.delete_page("a.md").unwrap();

    assert_eq!(recent_paths(&v, 10), ["c.md"]);
}

#[test]
fn recent_pages_reflects_rename_page() {
    let v = VaultBuilder::new().file("a.md", "").open();

    v.rename_page("a.md", "b").unwrap();

    assert_eq!(recent_paths(&v, 10), ["b.md"]);
}

#[test]
fn recent_pages_reflects_create_page() {
    let v = VaultBuilder::new()
        .file("a.md", "")
        .mtime("a.md", t(1_600_000_000))
        .open();

    v.create_page(None, "新規").unwrap();

    assert_eq!(recent_paths(&v, 10), ["新規.md", "a.md"]);
}

// ================================================================ 索引を消す・壊す・作り直す(仕様9・回答55・56)

/// 索引を消したり壊したりする前後で比べるための、いくつもの結果をまとめたもの。
fn observe(v: &Vault) -> (Vec<Vec<SearchHit>>, Vec<PageMeta>) {
    let queries = [
        "会議",
        "rust",
        "京都",
        "📝",
        "メモ",
        "定例",
        "ｒｕｓｔ",
        "会議 定例",
        "の",
    ];
    let hits = queries.iter().map(|q| v.search(q, 1000).unwrap()).collect();
    (hits, v.recent_pages(1000).unwrap())
}

fn sample_vault() -> TestVault {
    let mut b = VaultBuilder::new()
        .file("会議.md", "定例の会議 Rust の話")
        .file("会議/議事録.md", "京都タワー📝")
        .file("メモ.md", "rust RUST")
        .file(&nfd("がっこう.md"), "東京都の会議")
        .file("p/q/r.md", "定例")
        .file("空.md", "")
        .file("sjis.md", [0x82, 0xA0])
        .file(".hidden.md", "会議");
    let pages = [
        "会議.md",
        "会議/議事録.md",
        "メモ.md",
        "p/q/r.md",
        "空.md",
        "sjis.md",
    ];
    for (i, rel) in pages.iter().enumerate() {
        b = b.mtime(rel, t(1_600_000_000 + i as u64 * 1000));
    }
    b.open()
}

/// 閉じた状態で、index_dir の中のファイルをすべてでたらめな内容に書き換える。
fn garble_index(dir: &std::path::Path) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            garble_index(&p);
        } else {
            std::fs::write(&p, b"\x00\xFFgarbage \xDE\xAD\xBE\xEF not an index").unwrap();
        }
    }
}

#[test]
fn results_are_correct_after_reopening_without_index_and_without_rebuild() {
    let mut v = sample_vault();
    let before = observe(&v);

    v.delete_index_and_reopen();

    assert_eq!(observe(&v), before);
}

#[test]
fn results_are_correct_after_removing_index_dir_itself_and_reopening() {
    let mut v = sample_vault();
    let before = observe(&v);
    let index_dir = v.index_dir();

    v.delete_index_and_reopen_with(|| std::fs::remove_dir_all(&index_dir).unwrap());

    assert_eq!(observe(&v), before);
}

#[test]
fn results_are_same_after_deleting_index_and_rebuilding() {
    let mut v = sample_vault();
    let before = observe(&v);

    v.delete_index_and_reopen();
    v.rebuild_index().unwrap();

    assert_eq!(observe(&v), before);
}

#[test]
fn open_succeeds_and_results_are_correct_with_garbled_index() {
    let mut v = sample_vault();
    let before = observe(&v);
    let index_dir = v.index_dir();

    v.delete_index_and_reopen_with(|| garble_index(&index_dir));

    assert_eq!(observe(&v), before);
}

#[test]
fn rebuild_index_is_idempotent() {
    let v = sample_vault();
    v.rebuild_index().unwrap();
    let first = observe(&v);

    v.rebuild_index().unwrap();

    assert_eq!(observe(&v), first);
}

#[test]
fn rebuild_index_adds_nothing_to_vault() {
    let v = sample_vault();
    let before = v.all_entries();

    v.rebuild_index().unwrap();

    assert_eq!(v.all_entries(), before);
}

#[test]
fn rebuild_index_does_not_modify_pages() {
    let v = sample_vault();
    let before: Vec<_> = v
        .all_entries()
        .into_iter()
        .filter(|e| !e.ends_with('/'))
        .map(|e| (v.disk_bytes(&e), mtime(&v.root().join(&e)), e))
        .collect();

    v.rebuild_index().unwrap();

    for (bytes, m, rel) in before {
        assert_eq!(v.disk_bytes(&rel), bytes, "{rel}");
        assert_eq!(mtime(&v.root().join(&rel)), m, "{rel}");
    }
}
