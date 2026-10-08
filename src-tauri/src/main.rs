// リリース版の Windows でコンソールを出さない(macOS では影響しない)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    odin_app_lib::run()
}
