//! 大きな vault での性能測定。通常のテスト(`cargo test`)では走らない。
//! 実行: `cargo bench --bench large_vault`(リリース相当のビルドになる)
//!
//! 目安を超えても失敗にはせず、数値と目安を並べて表示する。
//! search / rebuild_index の測定は段階5で追加する。

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use odin_core::Vault;

const PAGES: usize = 5_000;
const PAGE_BYTES: usize = 2_048;
const RUNS: usize = 20;

/// 50 個の親ページ × 各 99 個の子ページ + 親 50 個 = 5,000 ページ、1ページ約 2KB。
fn build_large_vault(root: &Path) {
    let line = "これはベンチマーク用の本文です。東京都の天気、会議メモ、Lorem ipsum.\n";
    let body: String = line.repeat(PAGE_BYTES / line.len() + 1)[..]
        .chars()
        .take(PAGE_BYTES / 3)
        .collect();
    let parents = 50;
    let children = PAGES / parents - 1;
    for i in 0..parents {
        let parent = format!("ノート{i}");
        fs::write(root.join(format!("{parent}.md")), &body).unwrap();
        fs::create_dir(root.join(&parent)).unwrap();
        for j in 0..children {
            fs::write(root.join(&parent).join(format!("子{j}.md")), &body).unwrap();
        }
    }
}

fn median(mut xs: Vec<Duration>) -> Duration {
    xs.sort();
    xs[xs.len() / 2]
}

fn measure(mut f: impl FnMut()) -> Duration {
    f(); // 1回目は暖機として捨てる
    median(
        (0..RUNS)
            .map(|_| {
                let s = Instant::now();
                f();
                s.elapsed()
            })
            .collect(),
    )
}

fn report(name: &str, took: Duration, budget: Duration) {
    let verdict = if took <= budget {
        "目安内"
    } else {
        "目安超過"
    };
    println!(
        "{name:<28} 中央値 {:>9.2} ms  目安 {:>6} ms  {verdict}",
        took.as_secs_f64() * 1e3,
        budget.as_millis()
    );
}

fn main() {
    // `cargo bench` は `--bench` などの引数を渡してくるが、ここでは使わない。
    let vault_dir = tempfile::tempdir().unwrap();
    let index_dir = tempfile::tempdir().unwrap();
    build_large_vault(vault_dir.path());
    println!("vault: {PAGES} ページ、1ページ約 {PAGE_BYTES} バイト、各 {RUNS} 回の中央値");

    // 1回目の open で索引を作らせ、2回目以降を「索引が既にある状態の open」として測る。
    drop(Vault::open(vault_dir.path(), index_dir.path()).unwrap());
    let took = measure(|| drop(Vault::open(vault_dir.path(), index_dir.path()).unwrap()));
    report("open(索引あり)", took, Duration::from_millis(200));

    let vault = Vault::open(vault_dir.path(), index_dir.path()).unwrap();
    let took = measure(|| {
        vault.list_tree().unwrap();
    });
    report("list_tree", took, Duration::from_millis(50));
}
