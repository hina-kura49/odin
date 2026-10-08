//! 段階5: search / recent_pages / rebuild_index

mod common;

use std::time::{Duration, SystemTime};

use common::*;
use odin_core::{PageMeta, SearchHit, Vault};

fn t(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

fn search(v: &Vault, query: &str) -> Vec<SearchHit> {
    v.search(query).unwrap()
}

fn hit_paths(v: &Vault, query: &str) -> Vec<String> {
    search(v, query).into_iter().map(|h| h.path).collect()
}

fn hit_set(v: &Vault, query: &str) -> std::collections::BTreeSet<String> {
    hit_paths(v, query).into_iter().collect()
}

fn recent_paths(v: &Vault, limit: usize) -> Vec<String> {
    v.recent_pages(limit)
        .unwrap()
        .into_iter()
        .map(|m| m.path)
        .collect()
}

fn set(paths: &[&str]) -> std::collections::BTreeSet<String> {
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

// ================================================================ search: 日本語と英字

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
    assert!(v.search("京都").is_ok());
}

#[test]
fn search_finds_katakana_and_kanji_mixed_text() {
    let v = VaultBuilder::new()
        .file("a.md", "プロジェクト管理ツールの比較")
        .open();
    assert_eq!(hit_paths(&v, "ト管理ツ"), ["a.md"]);
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

// ================================================================ search: 並び順

#[test]
fn search_ranks_title_match_above_body_match() {
    let v = VaultBuilder::new()
        .file("メモ.md", "定例の会議について")
        .file("会議.md", "")
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
        .open();

    let paths = hit_paths(&v, "会議");

    assert_eq!(paths.len(), 4);
    assert_eq!(
        paths[..2]
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        set(&["会議録.md", "x/定例会議.md"])
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

// ================================================================ search: snippet

#[test]
fn snippet_contains_matched_text() {
    let v = VaultBuilder::new()
        .file("a.md", "前置き。キーワード。後ろ。")
        .open();

    let hits = search(&v, "キーワード");

    assert!(
        hits[0].snippet.contains("キーワード"),
        "{:?}",
        hits[0].snippet
    );
}

#[test]
fn snippet_contains_text_before_and_after_the_match() {
    let v = VaultBuilder::new()
        .file(
            "a.md",
            "前の文脈があります。キーワード。後の文脈があります。",
        )
        .open();

    let snippet = &search(&v, "キーワード")[0].snippet;

    assert!(snippet.contains("す。キーワード。後"), "{snippet:?}");
}

#[test]
fn snippet_keeps_original_letter_case_of_body() {
    let v = VaultBuilder::new().file("a.md", "これは Rust の話").open();

    let snippet = &search(&v, "rust")[0].snippet;

    assert!(snippet.contains("Rust"), "{snippet:?}");
}

#[test]
fn snippet_shows_match_near_the_end_of_a_large_file() {
    let body = format!("{}最後の目印です", "あいうえお\n".repeat(400_000));
    let v = VaultBuilder::new().file("big.md", body).open();

    let snippet = &search(&v, "最後の目印")[0].snippet;

    assert!(snippet.contains("最後の目印"), "{snippet:?}");
    assert!(
        snippet.len() < 10_000,
        "snippet が長すぎる: {} バイト",
        snippet.len()
    );
}

#[test]
fn snippet_does_not_split_characters() {
    // snippet は String なので、文字の途中で切れていれば作れない。絵文字の前後でも同じ。
    let v = VaultBuilder::new()
        .file(
            "a.md",
            format!("{}目印{}", "🍣".repeat(500), "👨‍👩‍👧".repeat(500)),
        )
        .open();

    let snippet = &search(&v, "目印")[0].snippet;

    assert!(snippet.contains("目印"), "{snippet:?}");
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
    v.rebuild_index().unwrap();

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

// ================================================================ search: アプリでの操作の反映

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

// ================================================================ search: アプリの外での変更(仕様9)

#[test]
fn search_reflects_page_added_outside_the_app_after_reopen() {
    let mut v = VaultBuilder::new().file("a.md", "").open();
    v.write_outside_app("p/new.md", "外で追加");

    v.reopen();

    assert_eq!(hit_paths(&v, "外で追加"), ["p/new.md"]);
}

#[test]
fn search_reflects_page_changed_outside_the_app_after_reopen() {
    let mut v = VaultBuilder::new().file("a.md", "変更前").open();
    search(&v, "変更前");
    v.write_outside_app("a.md", "変更後");
    v.set_mtime("a.md", t(1_900_000_000));

    v.reopen();

    assert!(search(&v, "変更前").is_empty());
    assert_eq!(hit_paths(&v, "変更後"), ["a.md"]);
}

#[test]
fn search_reflects_page_changed_outside_with_same_size_and_mtime_after_rebuild() {
    // 大きさも mtime も同じで中身だけ違う変更は、差分の再スキャンでは気づけないことがある。
    // rebuild_index ならファイルから作り直すので必ず反映される。
    let v = VaultBuilder::new().file("a.md", "内容A").open();
    v.set_mtime("a.md", t(1_600_000_000));
    search(&v, "内容A");
    v.write_outside_app("a.md", "内容B");
    v.set_mtime("a.md", t(1_600_000_000));

    v.rebuild_index().unwrap();

    assert!(search(&v, "内容A").is_empty());
    assert_eq!(hit_paths(&v, "内容B"), ["a.md"]);
}

#[test]
fn search_reflects_page_deleted_outside_the_app_after_reopen() {
    let mut v = VaultBuilder::new()
        .file("a.md", "合言葉")
        .file("b.md", "合言葉")
        .open();
    search(&v, "合言葉");
    std::fs::remove_file(v.root().join("a.md")).unwrap();

    v.reopen();

    assert_eq!(hit_paths(&v, "合言葉"), ["b.md"]);
}

#[test]
fn search_reflects_outside_changes_after_rebuild_index() {
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
    v.rebuild_index().unwrap();

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
fn recent_pages_reflects_mtime_changed_outside_after_reopen() {
    let mut v = VaultBuilder::new()
        .file("a.md", "")
        .file("b.md", "")
        .mtime("a.md", t(1_700_000_000))
        .mtime("b.md", t(1_600_000_000))
        .open();
    v.recent_pages(10).unwrap();
    v.set_mtime("b.md", t(1_800_000_000));

    v.reopen();

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

// ================================================================ rebuild_index(仕様9)

/// 索引を消して作り直す前後で比べるための、いくつもの結果をまとめたもの。
fn observe(v: &Vault) -> (Vec<Vec<SearchHit>>, Vec<PageMeta>) {
    let queries = ["会議", "rust", "京都", "📝", "メモ", "定例"];
    let hits = queries.iter().map(|q| v.search(q).unwrap()).collect();
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

#[test]
fn results_are_same_after_deleting_index_and_rebuilding() {
    let mut v = sample_vault();
    v.rebuild_index().unwrap();
    let before = observe(&v);

    v.delete_index_and_reopen();
    v.rebuild_index().unwrap();

    assert_eq!(observe(&v), before);
}

#[test]
fn results_are_same_after_removing_index_dir_itself_and_rebuilding() {
    let mut v = sample_vault();
    v.rebuild_index().unwrap();
    let before = observe(&v);
    let index_dir = v.index_dir();

    v.delete_index_and_reopen_with(|| std::fs::remove_dir_all(&index_dir).unwrap());
    v.rebuild_index().unwrap();

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

#[test]
fn index_files_live_only_in_index_dir() {
    let v = sample_vault();
    v.rebuild_index().unwrap();
    v.search("会議").unwrap();

    assert!(
        std::fs::read_dir(v.index_dir()).unwrap().next().is_some(),
        "index_dir に何も作られていない"
    );
}

// ================================================================ 仕様確認待ち

#[test]
#[ignore = "仕様確認待ち(質問45): 空の検索語や空白だけの検索語は、空の結果を返すか"]
fn search_with_empty_query_returns_nothing() {
    let v = VaultBuilder::new().file("a.md", "本文").open();
    assert!(search(&v, "").is_empty());
    assert!(search(&v, " \u{3000}").is_empty());
}

#[test]
#[ignore = "仕様確認待ち(質問46): 本文が NFD でも NFC の検索語で見つかるか"]
fn search_finds_nfd_body_by_nfc_query() {
    let v = VaultBuilder::new().file("a.md", nfd("がっこう")).open();
    assert_eq!(hit_paths(&v, &nfc("がっこう")), ["a.md"]);
}

#[test]
#[ignore = "仕様確認待ち(質問47): 全角英数字と半角英数字を同じとみなすか"]
fn search_treats_fullwidth_and_halfwidth_alphanumerics_alike() {
    let v = VaultBuilder::new().file("a.md", "ＡＢＣ１２３").open();
    assert_eq!(hit_paths(&v, "abc123"), ["a.md"]);
}
