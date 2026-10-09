//! 名前の扱い: 比べ方、並び順、タイトルの処理。

use std::cmp::Ordering;

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

/// タイトルの最大の長さ(NFC にしたときの UTF-8 のバイト数)。
const MAX_TITLE_BYTES: usize = 200;

/// 空のタイトルの代わりに使う名前。
pub(crate) const UNTITLED: &str = "無題";

pub(crate) fn nfc(s: &str) -> String {
    s.nfc().collect()
}

/// 名前どうしを比べるための形。macOS のファイル名と同じく、NFC にそろえて大文字小文字を区別しない。
pub(crate) fn key(s: &str) -> String {
    nfc(s).to_lowercase()
}

/// 名前に使えない文字(ディスク上にあれば、そのファイルやフォルダを隠す)。
fn is_unusable_in_name(c: char) -> bool {
    c == '\\' || c.is_control()
}

/// 一覧や検索に出してよい名前か。隠しファイルと、使えない文字を含む名前は出さない。
pub(crate) fn is_visible_name(name: &str) -> bool {
    !name.starts_with('.') && !name.chars().any(is_unusable_in_name)
}

/// 拡張子が .md(大文字小文字は問わない)の名前か。
pub(crate) fn is_md_name(name: &str) -> bool {
    name.len() > 3
        && name.is_char_boundary(name.len() - 3)
        && name[name.len() - 3..].eq_ignore_ascii_case(".md")
}

/// ページのファイル名から拡張子を除いた部分(ディスク上の表記のまま)。
pub(crate) fn stem(name: &str) -> &str {
    &name[..name.len() - 3]
}

/// 作る・名前を変えるときのタイトルの処理(回答27)。
/// 前後の空白を除き、NFC にし、使えない文字と先頭の「.」を「-」にし、200バイトに切り詰め、また前後の空白を除く。
pub(crate) fn process_title(raw: &str) -> String {
    let trimmed = nfc(raw.trim());

    // 「/」「:」「\」と制御文字は、1文字ずつ「-」にする。
    let replaced: String = trimmed
        .chars()
        .map(|c| {
            if c == '/' || c == ':' || is_unusable_in_name(c) {
                '-'
            } else {
                c
            }
        })
        .collect();

    // 先頭に続く「.」は、隠しファイルにならないよう、すべて「-」にする。
    let dots = replaced.chars().take_while(|&c| c == '.').count();
    let replaced = format!("{}{}", "-".repeat(dots), &replaced[dots..]);

    // 書記素の途中で切らないように、200バイトに収まるところまで残す。
    let mut truncated = String::new();
    for g in replaced.graphemes(true) {
        if truncated.len() + g.len() > MAX_TITLE_BYTES {
            break;
        }
        truncated.push_str(g);
    }

    let title = truncated.trim();
    if title.is_empty() {
        UNTITLED.to_string()
    } else {
        title.to_string()
    }
}

/// 数字(半角と全角)なら、その値を返す。
fn digit_value(c: char) -> Option<u32> {
    match c {
        '0'..='9' => Some(c as u32 - '0' as u32),
        '０'..='９' => Some(c as u32 - '０' as u32),
        _ => None,
    }
}

/// 並べるための単位。続いた数字はまとめて1つの数として扱う。
enum Token {
    Number(Vec<u32>),
    Char(char),
}

fn tokens(s: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut digits = Vec::new();
    for c in s.chars() {
        match digit_value(c) {
            Some(d) => digits.push(d),
            None => {
                if !digits.is_empty() {
                    out.push(Token::Number(std::mem::take(&mut digits)));
                }
                out.push(Token::Char(c));
            }
        }
    }
    if !digits.is_empty() {
        out.push(Token::Number(digits));
    }
    out
}

/// 数の大きさで比べる(先頭の0は無視する)。
fn compare_numbers(a: &[u32], b: &[u32]) -> Ordering {
    let a = &a[a.iter().take_while(|&&d| d == 0).count()..];
    let b = &b[b.iter().take_while(|&&d| d == 0).count()..];
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

/// 名前の自然な並び順(回答13・14)。NFC にそろえ、大文字小文字を区別せず、数字は数の大きさで比べる。
/// それでも同じなら、NFC にした名前の文字コードの順。
pub(crate) fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (a, b) = (nfc(a), nfc(b));
    let (ta, tb) = (tokens(&a.to_lowercase()), tokens(&b.to_lowercase()));
    for (x, y) in ta.iter().zip(&tb) {
        let order = match (x, y) {
            (Token::Number(x), Token::Number(y)) => compare_numbers(x, y),
            // 数と文字を比べるときは、数を「0」として比べる(半角と全角で同じ位置になる)。
            (Token::Number(_), Token::Char(y)) => '0'.cmp(y),
            (Token::Char(x), Token::Number(_)) => x.cmp(&'0'),
            (Token::Char(x), Token::Char(y)) => x.cmp(y),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    ta.len().cmp(&tb.len()).then_with(|| a.cmp(&b))
}

/// path(「/」区切り)の自然な並び順。フォルダごとに natural_cmp で比べる。
pub(crate) fn natural_path_cmp(a: &str, b: &str) -> Ordering {
    let mut xs = a.split('/');
    let mut ys = b.split('/');
    loop {
        match (xs.next(), ys.next()) {
            (Some(x), Some(y)) => {
                let order = natural_cmp(x, y);
                if order != Ordering::Equal {
                    return order;
                }
            }
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}
