//! 本物の macOS のゴミ箱(`SystemTrash`)を使うテスト。
//!
//! 利用者のゴミ箱にファイルが増えるので、通常は走らせない。
//! 実行: `cargo test --test system_trash -- --ignored`

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use odin_core::Vault;

#[test]
#[ignore = "利用者のゴミ箱を使うため、明示的に実行する"]
fn delete_page_moves_page_and_children_to_macos_trash() {
    let vault = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let name = format!("odin-core-trash-test-{stamp}");
    fs::write(vault.path().join(format!("{name}.md")), "親").unwrap();
    fs::create_dir(vault.path().join(&name)).unwrap();
    fs::write(vault.path().join(&name).join("子.md"), "子").unwrap();

    let v = Vault::open(vault.path(), index.path()).unwrap();
    v.delete_page(&format!("{name}.md")).unwrap();

    assert_eq!(fs::read_dir(vault.path()).unwrap().count(), 0);

    // ゴミ箱の中を読むにはフルディスクアクセスが要る場合がある。読めたときだけ確かめて片付ける。
    let trash = std::env::home_dir().unwrap().join(".Trash");
    match fs::read_dir(&trash) {
        Ok(entries) => {
            let found: Vec<_> = entries
                .map(|e| e.unwrap().path())
                .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(&name))
                .collect();
            assert!(!found.is_empty(), "ゴミ箱に {name} が見つからない");
            for p in found {
                if p.is_dir() {
                    fs::remove_dir_all(p).unwrap()
                } else {
                    fs::remove_file(p).unwrap()
                }
            }
        }
        Err(e) => eprintln!("ゴミ箱を読めないため、ゴミ箱側の確認を省いた: {e}"),
    }
}
