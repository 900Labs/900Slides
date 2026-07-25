#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    let _version = slides_core::version();
    tauri::Builder::default()
        .manage(commands::AppState::new())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::new_deck,
            commands::open_deck,
            commands::save_deck,
            commands::get_snapshot,
            commands::edit_text,
            commands::edit_text_box,
            commands::insert_image,
            commands::add_shape,
            commands::update_shape_transform,
            commands::update_shape_style,
            commands::delete_shape,
            commands::render_slide_svg,
            commands::undo,
            commands::get_loss_ledger,
            commands::start_presenter,
            commands::get_presenter_state,
            commands::presenter_next,
            commands::presenter_previous,
            commands::list_recovery_snapshots,
            commands::restore_recovery,
            commands::discard_recovery,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
