//! Odin のデスクトップアプリの殻。
//! いまはフロントエンドを MockBackend のまま動かす。core/ はまだつながない。
//!
//! ウィンドウは2つ(tauri.conf.json):
//! - main: ノートの画面。閉じても隠すだけで、アプリは動き続ける(Dock のアイコンで出し直す。終了は ⌘Q)
//! - capture: クイックキャプチャ。起動時に隠して作っておき、グローバルホットキーで出す(出すときに読み込みを待たせない)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const MAIN_LABEL: &str = "main";
const CAPTURE_LABEL: &str = "capture";
const QUIT_MENU_ID: &str = "quit";

/// 終了の前に、メインのウィンドウに未保存の変更を保存し終えてもらうためのイベント(フロントは finish_quit で答える)
const QUIT_REQUESTED_EVENT: &str = "app:quit-requested";
/// メインのウィンドウを閉じる要求(閉じるボタン・⌘W)。フロントが保存してから隠す
const HIDE_REQUESTED_EVENT: &str = "app:hide-requested";
/// フロントが答えないとき(固まっているなど)に、そのまま終了するまでの時間
const QUIT_FALLBACK: Duration = Duration::from_secs(10);

/// クイックキャプチャのホットキー。フロントエンドの表示と同じファイルから読む(変えるときはこのファイルだけを変える)。
/// ⌃Space は macOS の入力ソースの切り替えと重なるので使わない。
const CAPTURE_SHORTCUT_JSON: &str = include_str!("../../src/capture/shortcut.json");

#[derive(Deserialize)]
struct CaptureShortcut {
    accelerator: String,
}

#[derive(Default)]
struct AppState {
    /// ホットキーを登録できなかったときの理由。メインのウィンドウが起動後に読みに来る(起動前に知らせると取りこぼすので)
    shortcut_error: Mutex<Option<String>>,
    /// 終了の途中か(⌘Q を続けて押しても、保存を一度だけ頼む)
    quitting: AtomicBool,
}

/// ホットキーを登録できなかったか。登録できていれば None
#[tauri::command]
fn capture_shortcut_error(state: State<'_, AppState>) -> Option<String> {
    state.shortcut_error.lock().ok().and_then(|e| e.clone())
}

/// 終了の前の保存が終わった。exit が false のとき(保存できなかったとき)は終了をやめて、メインのウィンドウを出す
#[tauri::command]
fn finish_quit(app: AppHandle, state: State<'_, AppState>, exit: bool) {
    if !state.quitting.swap(false, Ordering::SeqCst) {
        return;
    }
    if exit {
        app.exit(0);
    } else {
        show_main(&app);
    }
}

/// ⌘Q: メインのウィンドウに保存を頼み、答えを待ってから終了する
fn request_quit(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.quitting.swap(true, Ordering::SeqCst) {
        return;
    }
    if app.emit(QUIT_REQUESTED_EVENT, ()).is_err() {
        app.exit(0);
        return;
    }
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(QUIT_FALLBACK);
        if app.state::<AppState>().quitting.load(Ordering::SeqCst) {
            app.exit(0);
        }
    });
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
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

/// Tauri の既定のメニューから、終了だけを自前の項目に替えたもの。
/// 既定の「終了」は macOS がその場でアプリを終えるので、保存を待てない。
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let name = app.package_info().name.clone();
    let about = AboutMetadata {
        name: Some(name.clone()),
        version: Some(app.package_info().version.to_string()),
        ..Default::default()
    };
    let quit = MenuItem::with_id(app, QUIT_MENU_ID, format!("{name} を終了"), true, Some("CmdOrCtrl+Q"))?;
    Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                &name,
                true,
                &[
                    &PredefinedMenuItem::about(app, None, Some(about))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, None)?,
                    &PredefinedMenuItem::hide_others(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &quit,
                ],
            )?,
            &Submenu::with_items(app, "File", true, &[&PredefinedMenuItem::close_window(app, None)?])?,
            &Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None)?,
                    &PredefinedMenuItem::redo(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, None)?,
                    &PredefinedMenuItem::copy(app, None)?,
                    &PredefinedMenuItem::paste(app, None)?,
                    &PredefinedMenuItem::select_all(app, None)?,
                ],
            )?,
            &Submenu::with_items(app, "View", true, &[&PredefinedMenuItem::fullscreen(app, None)?])?,
            &Submenu::with_items(
                app,
                "Window",
                true,
                &[
                    &PredefinedMenuItem::minimize(app, None)?,
                    &PredefinedMenuItem::maximize(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::close_window(app, None)?,
                ],
            )?,
        ],
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shortcut: CaptureShortcut = serde_json::from_str(CAPTURE_SHORTCUT_JSON)
        .expect("src/capture/shortcut.json の形が正しくありません");

    let app = tauri::Builder::default()
        .manage(AppState::default())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_capture(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![capture_shortcut_error, finish_quit])
        .menu(build_menu)
        .on_menu_event(|app, event| {
            if event.id() == QUIT_MENU_ID {
                request_quit(app);
            }
        })
        .setup(move |app| {
            // ほかのアプリが同じホットキーを使っているときは登録できない。アプリは止めずに、メインのウィンドウで知らせる
            if let Err(e) = app
                .global_shortcut()
                .register(shortcut.accelerator.as_str())
            {
                eprintln!(
                    "クイックキャプチャのホットキー {} を登録できませんでした: {e}",
                    shortcut.accelerator
                );
                if let Ok(mut slot) = app.state::<AppState>().shortcut_error.lock() {
                    *slot = Some(e.to_string());
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // どちらのウィンドウも閉じずに隠す(グローバルホットキーとクイックキャプチャを動かし続ける)。
            // - クイックキャプチャ: その場で隠す(次に出すときに待たせない)
            // - メイン: フロントに頼み、未保存の変更を保存し終えてから隠してもらう
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == CAPTURE_LABEL {
                    let _ = window.hide();
                } else if window.label() == MAIN_LABEL {
                    let _ = window.emit_to(MAIN_LABEL, HIDE_REQUESTED_EVENT, ());
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("アプリを起動できませんでした");

    app.run(|app, event| {
        // Dock のアイコンを押したら、隠したメインのウィンドウを出し直す
        if let RunEvent::Reopen { .. } = event {
            show_main(app);
        }
    });
}
