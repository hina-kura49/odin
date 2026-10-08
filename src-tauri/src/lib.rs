//! Odin のデスクトップアプリの殻。
//! いまはフロントエンドを MockBackend のまま動かす。core/ はまだつながない。
//!
//! ウィンドウは2つ(tauri.conf.json):
//! - main: ノートの画面
//! - capture: クイックキャプチャ。起動時に隠して作っておき、グローバルホットキーで出す(出すときに読み込みを待たせない)

use serde::Deserialize;
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const MAIN_LABEL: &str = "main";
const CAPTURE_LABEL: &str = "capture";

/// クイックキャプチャのホットキー。フロントエンドの表示と同じファイルから読む(変えるときはこのファイルだけを変える)。
/// ⌃Space は macOS の入力ソースの切り替えと重なるので使わない。
const CAPTURE_SHORTCUT_JSON: &str = include_str!("../../src/capture/shortcut.json");

#[derive(Deserialize)]
struct CaptureShortcut {
    accelerator: String,
}

/// クイックキャプチャを画面中央に出して、入力できる状態にする。出ていて手前にあれば隠す
fn toggle_capture(app: &AppHandle) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    let _ = window.center();
    let _ = window.show();
    let _ = window.set_focus();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shortcut: CaptureShortcut = serde_json::from_str(CAPTURE_SHORTCUT_JSON)
        .expect("src/capture/shortcut.json の形が正しくありません");

    let app = tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_capture(app);
                    }
                })
                .build(),
        )
        .setup(move |app| {
            // ほかのアプリが同じホットキーを使っているときは登録できない。アプリは止めずに知らせる
            if let Err(e) = app
                .global_shortcut()
                .register(shortcut.accelerator.as_str())
            {
                eprintln!(
                    "クイックキャプチャのホットキー {} を登録できませんでした: {e}",
                    shortcut.accelerator
                );
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // クイックキャプチャのウィンドウは閉じずに隠す(次に出すときに待たせない)
            if window.label() == CAPTURE_LABEL {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("アプリを起動できませんでした");

    app.run(|app, event| {
        // メインのウィンドウを閉じたら、隠れているクイックキャプチャごとアプリを終える
        if let RunEvent::WindowEvent {
            label,
            event: WindowEvent::Destroyed,
            ..
        } = &event
        {
            if label == MAIN_LABEL {
                app.exit(0);
            }
        }
    });
}
