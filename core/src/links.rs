//! rename_page でのリンクの書き換え(回答33〜44)。
//!
//! 名前を変えるページ、またはその子フォルダの中のものを指していたリンクが、名前を変えた後も
//! 同じものを指すように、行き先のうち変わった部分だけを書き換える。
//! 対象は通常のリンク・画像・参照定義。HTML の属性、`<a.md>`、コード、フロントマター、HTML コメントは対象外。

use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

use crate::names::{key, nfc};

/// 名前を変えるページについての情報。名前はすべて比べるための形(`key`)。
pub(crate) struct Target {
    /// ページのあるフォルダの、ルートからの名前
    pub parent: Vec<String>,
    /// ページのファイル名
    pub page: String,
    /// 子フォルダの名前(子フォルダがあるときだけ)
    pub folder: Option<String>,
    /// 元のタイトル(NFC)
    pub old_title: String,
    /// 新しいタイトル(処理済み)
    pub new_title: String,
}

/// リンクの表示文字列で「\」を付ける文字(GFM で意味を持つもの)。
const TEXT_ESCAPES: &[char] = &['\\', '[', ']', '*', '_', '`', '<', '&', '~', '|'];

/// 新しいタイトルを、リンクの表示文字列として書ける形にする。
fn escape_text(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if TEXT_ESCAPES.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// 新しい名前を、リンクの行き先として書ける形にする(回答37)。
/// ASCII の英数字と「- _ . /」はそのまま、ほかの ASCII は %XX(大文字)。
/// ASCII 以外は、空白と制御文字だけを %XX にする。
fn encode_dest(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        let raw = if c.is_ascii() {
            c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/')
        } else {
            !(c.is_whitespace() || c.is_control())
        };
        if raw {
            out.push(c);
        } else {
            let mut buf = [0; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// %XX を戻す(大文字小文字を問わない)。UTF-8 にならなければ None。
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
            && let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}

/// 「\」による ASCII の記号のエスケープを戻す。
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek().is_some_and(|n| n.is_ascii_punctuation()) {
            out.push(chars.next().unwrap());
        } else {
            out.push(c);
        }
    }
    out
}

/// URL のスキーム(`https:` など)で始まるか。
fn has_scheme(dest: &str) -> bool {
    let mut chars = dest.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    for c in chars {
        if c == ':' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
            return false;
        }
    }
    false
}

/// 行き先(元の書き方のまま)のうち、書き換える部分。
enum Change {
    /// ページ自身を指す。この範囲(行き先の中のバイト位置)のファイル名を書き換える
    Page(Range<usize>),
    /// 子フォルダの中を指す。この範囲のフォルダ名を書き換える
    Folder(Range<usize>),
}

/// 行き先が名前を変えるページを指していれば、書き換える部分を返す。
/// `base` はリンクを含むページのあるフォルダ(比べるための形)。
fn classify(raw: &str, base: &[String], target: &Target) -> Option<Change> {
    if raw.is_empty() || raw.starts_with('/') || raw.starts_with('#') || has_scheme(raw) {
        return None;
    }
    let path_end = raw.find(['#', '?']).unwrap_or(raw.len());
    let path = &raw[..path_end];

    // 行き先をたどる。それぞれの名前が、行き先のどこに書かれていたか(フォルダから受け継いだものは None)。
    let mut stack: Vec<(String, Option<Range<usize>>)> =
        base.iter().map(|k| (k.clone(), None)).collect();
    let mut pos = 0;
    for segment in path.split('/') {
        let range = pos..pos + segment.len();
        pos += segment.len() + 1;
        let decoded = percent_decode(&unescape(segment))?;
        match decoded.as_str() {
            "" | "." => {}
            ".." => {
                stack.pop()?;
            }
            name => stack.push((key(name), Some(range))),
        }
    }

    let depth = target.parent.len();
    if stack.len() <= depth
        || stack[..depth]
            .iter()
            .map(|(k, _)| k)
            .ne(target.parent.iter())
    {
        return None;
    }
    let (name, range) = &stack[depth];
    if stack.len() == depth + 1 && *name == target.page {
        return range.clone().map(Change::Page);
    }
    if target.folder.as_ref() == Some(name) {
        // 子フォルダの中から子フォルダの中を指すリンクは、一緒に動くので書き換えない。
        return range.clone().map(Change::Folder);
    }
    None
}

/// リンクの行き先(元の書き方)を読み取る。`at` は「(」や「:」の後の位置。
/// 戻り値は、行き先そのもの(`<>` を除く)の範囲。
fn read_dest(text: &str, at: usize) -> Option<Range<usize>> {
    let bytes = text.as_bytes();
    let mut i = at;
    // 空白と、多くても1つの改行を飛ばす。
    let mut newlines = 0;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
        if bytes[i] == b'\n' {
            newlines += 1;
        }
        i += 1;
    }
    if newlines > 1 {
        return None;
    }
    if bytes.get(i) == Some(&b'<') {
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() {
            match bytes[j] {
                b'\\' => j += 2,
                b'>' => return Some(start..j),
                b'\n' | b'<' => return None,
                _ => j += 1,
            }
        }
        return None;
    }
    let start = i;
    let mut depth = 0usize;
    let mut j = start;
    while j < bytes.len() {
        match bytes[j] {
            b'\\' if bytes.get(j + 1).is_some_and(u8::is_ascii_punctuation) => j += 2,
            b'(' => {
                depth += 1;
                j += 1;
            }
            b')' if depth == 0 => break,
            b')' => {
                depth -= 1;
                j += 1;
            }
            c if c.is_ascii_whitespace() || c.is_ascii_control() => break,
            _ => j += 1,
        }
    }
    Some(start..j.min(bytes.len()))
}

/// 書き換え1つ。`text` の `range` を `with` に置き換える。
struct Edit {
    range: Range<usize>,
    with: String,
}

/// 行き先の書き換えを1つ作る。`dest` は本文の中の行き先の範囲、`parsed` は解析器が読んだ行き先。
fn dest_edit(
    text: &str,
    dest: Range<usize>,
    parsed: &str,
    base: &[String],
    target: &Target,
) -> Option<(Edit, bool)> {
    let raw = &text[dest.clone()];
    // 自分で読み取った行き先が解析器と食い違うときは、触らない。
    if unescape(raw) != parsed {
        return None;
    }
    let (range, with, is_page) = match classify(raw, base, target)? {
        Change::Page(r) => (r, format!("{}.md", encode_dest(&target.new_title)), true),
        Change::Folder(r) => (r, encode_dest(&target.new_title), false),
    };
    let edit = Edit {
        range: dest.start + range.start..dest.start + range.end,
        with,
    };
    Some((edit, is_page))
}

/// 1ページの本文の中のリンクを書き換える。変わらなければ None。
/// `base` はそのページのあるフォルダ(名前を変える前の場所、比べるための形)。
pub(crate) fn rewrite(content: &str, base: &[String], target: &Target) -> Option<String> {
    // BOM は解析の前に外し、位置をずらして戻す。
    let offset = if content.starts_with('\u{FEFF}') {
        '\u{FEFF}'.len_utf8()
    } else {
        0
    };
    let text = &content[offset..];
    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let parser = Parser::new_ext(text, options);

    let mut edits = Vec::new();

    // 参照定義: 「[ラベル]: 行き先」
    for (_, def) in parser.reference_definitions().iter() {
        let span = &text[def.span.clone()];
        let Some(colon) = definition_colon(span) else {
            continue;
        };
        let Some(dest) = read_dest(text, def.span.start + colon + 1) else {
            continue;
        };
        if let Some((edit, _)) = dest_edit(text, dest, &def.dest, base, target) {
            edits.push(edit);
        }
    }

    // 通常のリンクと画像: 「[表示](行き先)」「![代替](行き先)」
    struct Open {
        start: usize,
        is_image: bool,
        inline: bool,
        dest: String,
        inner_end: usize,
        plain: bool,
        text: String,
    }
    let mut open: Vec<Open> = Vec::new();
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            })
            | Event::Start(Tag::Image {
                link_type,
                dest_url,
                ..
            }) => {
                // 外側のリンクにとっては、このリンク全体が中身。
                for o in open.iter_mut() {
                    o.inner_end = o.inner_end.max(range.end);
                    o.plain = false;
                }
                let is_image = text[range.start..].starts_with('!');
                open.push(Open {
                    start: range.start,
                    is_image,
                    inline: link_type == LinkType::Inline,
                    dest: dest_url.to_string(),
                    inner_end: range.start + if is_image { 2 } else { 1 },
                    plain: true,
                    text: String::new(),
                });
            }
            Event::End(TagEnd::Link) | Event::End(TagEnd::Image) => {
                let Some(o) = open.pop() else {
                    continue;
                };
                if !o.inline {
                    continue;
                }
                // 表示文字列の後には「](」が続く。
                let close = o.inner_end;
                if !text[close..].starts_with("](") {
                    continue;
                }
                let Some(dest) = read_dest(text, close + 2) else {
                    continue;
                };
                let Some((edit, is_page)) = dest_edit(text, dest, &o.dest, base, target) else {
                    continue;
                };
                edits.push(edit);
                // ページ自身へのリンクで、表示文字列が元のタイトルと同じなら、新しいタイトルにする(回答33)。
                if is_page && !o.is_image && o.plain && nfc(&o.text) == target.old_title {
                    edits.push(Edit {
                        range: o.start + 1..close,
                        with: escape_text(&target.new_title),
                    });
                }
            }
            Event::Text(t) => {
                for o in open.iter_mut() {
                    o.inner_end = o.inner_end.max(range.end);
                }
                if let Some(o) = open.last_mut() {
                    o.text.push_str(&t);
                }
            }
            _ => {
                // 強調・コード・取り消し線などがあれば、表示文字列はタイトルと同じとみなさない。
                for o in open.iter_mut() {
                    o.inner_end = o.inner_end.max(range.end);
                    o.plain = false;
                }
            }
        }
    }

    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|e| e.range.start);
    let mut out = String::with_capacity(content.len());
    out.push_str(&content[..offset]);
    let mut pos = 0;
    for edit in edits {
        if edit.range.start < pos {
            continue;
        }
        out.push_str(&text[pos..edit.range.start]);
        out.push_str(&edit.with);
        pos = edit.range.end;
    }
    out.push_str(&text[pos..]);
    (out != content).then_some(out)
}

/// 参照定義の中で、ラベルの後の「:」の位置。
fn definition_colon(span: &str) -> Option<usize> {
    let bytes = span.as_bytes();
    let mut i = span.find('[')? + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b']' => return (bytes.get(i + 1) == Some(&b':')).then_some(i + 1),
            _ => i += 1,
        }
    }
    None
}
