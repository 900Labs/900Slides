#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod versions;

use tauri::Manager;

fn main() {
    let _version = slides_core::version();
    tauri::Builder::default()
        .manage(commands::AppState::new())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Load any persisted user-dictionary words into the spell checker
            // before the UI is interactive. Missing file -> empty user dict.
            let state = app.state::<commands::AppState>();
            state.load_user_dictionary();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::new_deck,
            commands::open_deck,
            commands::save_deck,
            commands::get_snapshot,
            commands::edit_text,
            commands::edit_text_box,
            commands::set_run_style,
            commands::set_paragraph_style,
            commands::insert_image,
            commands::add_shape,
            commands::update_shape_transform,
            commands::update_shape_style,
            commands::delete_shape,
            commands::add_table,
            commands::set_cell_text,
            commands::set_cell_style,
            commands::insert_row,
            commands::insert_column,
            commands::delete_row,
            commands::delete_column,
            commands::add_chart,
            commands::set_chart_type,
            commands::set_chart_data,
            commands::set_chart_title,
            commands::set_transition,
            commands::set_slide_animation,
            commands::add_build_step,
            commands::remove_build_step,
            commands::move_build_step,
            commands::set_slide_size,
            commands::set_sections,
            commands::set_rich_notes,
            commands::set_high_contrast,
            commands::set_presenter_settings,
            commands::set_template,
            commands::set_slide_layout,
            commands::list_templates,
            commands::render_slide_svg,
            commands::export_svg,
            commands::export_png,
            commands::export_pdf,
            commands::compute_morph,
            commands::undo,
            commands::get_loss_ledger,
            commands::start_presenter,
            commands::get_presenter_state,
            commands::presenter_next,
            commands::presenter_previous,
            commands::list_recovery_snapshots,
            commands::restore_recovery,
            commands::discard_recovery,
            commands::spell_check,
            commands::spell_suggest,
            commands::spell_add_word,
            commands::list_versions,
            commands::get_version,
            commands::restore_version,
            commands::name_version,
            commands::diff_versions,
            commands::add_comment,
            commands::reply_to_comment,
            commands::set_comment_resolved,
            commands::assign_comment,
            commands::delete_comment_thread,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
