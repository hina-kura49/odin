use std::collections::BTreeSet;
use std::fs;

fn main() {
    if std::env::var_os("CARGO_FEATURE_CORE").is_some() {
        check_core_error_kinds();
    }
    tauri_build::build()
}

/// core の Error の種類が、src/ipc.rs のエラーの対応づけと食い違っていたらビルドを止める。
///
/// core の Error は #[non_exhaustive] なので、ほかのクレートからの match には `_` の腕が要り、
/// 種類が増えてもコンパイラは知らせてくれない。そこで、両方のソースを読んで種類の名前を突き合わせる。
fn check_core_error_kinds() {
    const CORE: &str = "../core/src/lib.rs";
    const IPC: &str = "src/ipc.rs";
    println!("cargo:rerun-if-changed={CORE}");
    println!("cargo:rerun-if-changed={IPC}");

    let core = fs::read_to_string(CORE).expect("core/src/lib.rs を読めません");
    let ipc = fs::read_to_string(IPC).expect("src/ipc.rs を読めません");

    let in_core = core_error_variants(&core);
    let in_ipc: BTreeSet<String> = ipc
        .split("odin_core::Error::")
        .skip(1)
        .filter_map(leading_ident)
        .collect();

    assert!(!in_core.is_empty(), "core/src/lib.rs に `pub enum Error` が見つかりません");
    if in_core != in_ipc {
        let missing: Vec<_> = in_core.difference(&in_ipc).collect();
        let extra: Vec<_> = in_ipc.difference(&in_core).collect();
        panic!(
            "core の Error の種類と src/ipc.rs の対応づけが食い違っています。\n\
             対応づけのない種類: {missing:?}\n\
             core にない種類: {extra:?}\n\
             src/ipc.rs の ErrorKind と From<odin_core::Error> を直し、契約の BackendErrorKind との違いを確かめてください。"
        );
    }
}

/// `pub enum Error { ... }` の中の種類の名前
fn core_error_variants(source: &str) -> BTreeSet<String> {
    let Some(start) = source.find("pub enum Error") else {
        return BTreeSet::new();
    };
    let body = &source[start..];
    let open = body.find('{').map(|i| i + 1).unwrap_or(0);
    let mut depth = 1;
    let mut end = open;
    for (i, c) in body[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    body[open..end]
        .lines()
        .map(str::trim)
        // コメントと属性(#[error(...)] など)を除く
        .filter(|line| !line.starts_with("//") && !line.starts_with('#'))
        .filter_map(leading_ident)
        .filter(|name| name.starts_with(|c: char| c.is_ascii_uppercase()))
        .collect()
}

fn leading_ident(s: &str) -> Option<String> {
    let ident: String = s.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    (!ident.is_empty()).then_some(ident)
}
