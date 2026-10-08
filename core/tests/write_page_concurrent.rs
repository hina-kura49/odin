//! 段階2: 書き込み中に別のスレッドが読んでも、中途半端な内容が見えないこと。
//!
//! スレッドの進み方に依存するので通常のテストとは分けている。正しい実装なら必ず通る
//! (時間で失敗することはない)が、誤った実装を必ず捕まえられるとは限らない。

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use common::*;
use odin_core::WriteResult;

#[test]
fn concurrent_reader_never_sees_partial_content() {
    let a = "A".repeat(2_000_000);
    let b = "B".repeat(2_000_000);
    let v = VaultBuilder::new().file("a.md", &a).open();
    let path = v.root().join("a.md");
    let done = Arc::new(AtomicBool::new(false));

    let reader = {
        let (path, done, a, b) = (path.clone(), done.clone(), a.clone(), b.clone());
        std::thread::spawn(move || {
            let mut reads = 0;
            while !done.load(Ordering::Relaxed) {
                let seen = std::fs::read(&path).unwrap();
                assert!(
                    seen == a.as_bytes() || seen == b.as_bytes(),
                    "途中の内容が見えた({} バイト)",
                    seen.len()
                );
                reads += 1;
            }
            reads
        })
    };

    let mut version = v.version("a.md");
    for i in 0..30 {
        let next = if i % 2 == 0 { &b } else { &a };
        match v.write_page("a.md", next, &version).unwrap() {
            WriteResult::Ok { version: new } => version = new,
            WriteResult::Conflict => panic!("Conflict になった"),
        }
    }
    done.store(true, Ordering::Relaxed);

    assert!(reader.join().unwrap() > 0);
}
