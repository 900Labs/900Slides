#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let _version = slides_core::version();
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
