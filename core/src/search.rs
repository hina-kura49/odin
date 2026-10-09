//! 検索のための文字の比べ方と、抜粋(snippet)の作り方。

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

use crate::Snippet;

/// 抜粋で一致の前に置く最大の文字数(書記素)。
const BEFORE_MAX: usize = 30;
/// 抜粋の全体の最大の文字数(書記素)。「…」は数えない。
const TOTAL_MAX: usize = 120;
const ELLIPSIS: &str = "…";

/// 1つの書記素を、比べるための形にする。NFKC にそろえ、大文字小文字を区別しない。
fn fold_grapheme(g: &str, out: &mut String) {
    if g.is_ascii() {
        out.extend(g.chars().map(|c| c.to_ascii_lowercase()));
    } else {
        for c in g.nfkc() {
            out.extend(c.to_lowercase());
        }
    }
}

/// 文字列を、比べるための形にする。書記素ごとに変えるので、抜粋の位置と対応がとれる。
pub(crate) fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for g in s.graphemes(true) {
        fold_grapheme(g, &mut out);
    }
    out
}

/// 検索語を、空白で区切った語の並びにする(比べるための形)。
pub(crate) fn query_words(query: &str) -> Vec<String> {
    fold(query).split_whitespace().map(str::to_string).collect()
}

/// 抜粋を作るための、本文の1文字(書記素)。続いた空白は1つの半角空白にまとめてある。
struct Cell<'a> {
    text: &'a str,
    /// 比べるための形での、この文字の始まりの位置
    folded_start: usize,
}

/// 本文を、空白をまとめて前後の空白を除いた文字の並びと、比べるための形の文字列にする。
fn cells(body: &str) -> (Vec<Cell<'_>>, String) {
    let body = body.strip_prefix('\u{FEFF}').unwrap_or(body);
    let mut cells: Vec<Cell> = Vec::new();
    let mut folded = String::new();
    let mut in_space = false;
    for g in body.graphemes(true) {
        if g.chars().next().is_some_and(char::is_whitespace) {
            in_space = true;
            continue;
        }
        if in_space && !cells.is_empty() {
            cells.push(Cell {
                text: " ",
                folded_start: folded.len(),
            });
            folded.push(' ');
        }
        in_space = false;
        cells.push(Cell {
            text: g,
            folded_start: folded.len(),
        });
        fold_grapheme(g, &mut folded);
    }
    (cells, folded)
}

fn join(cells: &[Cell]) -> String {
    cells.iter().map(|c| c.text).collect()
}

/// 本文から抜粋を作る(回答51)。`words` のうち本文で最初に現れる一致を中心にする。
/// どの語も本文になければ、タイトルだけの一致として本文の先頭を `after` に入れる。
pub(crate) fn snippet(body: &str, words: &[String]) -> Snippet {
    let (cells, folded) = cells(body);

    let first = words
        .iter()
        .filter_map(|w| {
            folded
                .find(w.as_str())
                .map(|start| (start, start + w.len()))
        })
        .min();
    let Some((start, end)) = first else {
        let shown = cells.len().min(TOTAL_MAX);
        let mut after = join(&cells[..shown]);
        if shown < cells.len() {
            after.push_str(ELLIPSIS);
        }
        return Snippet {
            before: String::new(),
            hit: String::new(),
            after,
        };
    };

    // 一致の始まりと終わりを含む文字を探す。
    let cell_at = |pos: usize| cells.partition_point(|c| c.folded_start <= pos) - 1;
    let first_cell = cell_at(start);
    let last_cell = cell_at(end - 1);

    let before_start = first_cell.saturating_sub(BEFORE_MAX);
    let mut before = join(&cells[before_start..first_cell]);
    if before_start > 0 {
        before.insert_str(0, ELLIPSIS);
    }
    let hit_cells = &cells[first_cell..=last_cell];
    let hit = join(hit_cells);

    let room = TOTAL_MAX.saturating_sub((first_cell - before_start) + hit_cells.len());
    let after_end = (last_cell + 1 + room).min(cells.len());
    let mut after = join(&cells[last_cell + 1..after_end]);
    if after_end < cells.len() {
        after.push_str(ELLIPSIS);
    }
    Snippet { before, hit, after }
}
