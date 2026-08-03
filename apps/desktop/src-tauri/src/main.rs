#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod versions;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    Emitter, Manager,
};

fn main() {
    let _version = slides_core::version();

    tauri::Builder::default()
        .setup(|app| {
            let new_item = MenuItem::with_id(app, "menu_new", "New", true, Some("CmdOrCtrl+N"))?;
            let open_item =
                MenuItem::with_id(app, "menu_open", "Open…", true, Some("CmdOrCtrl+O"))?;
            let save_item =
                MenuItem::with_id(app, "menu_save", "Save", true, Some("CmdOrCtrl+S"))?;
            let save_as_item = MenuItem::with_id(
                app,
                "menu_save_as",
                "Save As…",
                true,
                Some("CmdOrCtrl+Shift+S"),
            )?;
            let export_svg_item =
                MenuItem::with_id(app, "menu_export_svg", "Export as SVG", true, None::<&str>)?;
            let export_png_item =
                MenuItem::with_id(app, "menu_export_png", "Export as PNG", true, None::<&str>)?;
            let export_pdf_item =
                MenuItem::with_id(app, "menu_export_pdf", "Export as PDF", true, None::<&str>)?;
            let export_sub = Submenu::with_items(
                app,
                "Export",
                true,
                &[&export_svg_item, &export_png_item, &export_pdf_item],
            )?;
            let file_menu = Submenu::with_items(
                app,
                "File",
                true,
                &[
                    &new_item,
                    &open_item,
                    &save_item,
                    &save_as_item,
                    &export_sub,
                ],
            )?;

            let undo_item =
                MenuItem::with_id(app, "menu_undo", "Undo", true, Some("CmdOrCtrl+Z"))?;
            let redo_item = MenuItem::with_id(
                app,
                "menu_redo",
                "Redo",
                true,
                Some("CmdOrCtrl+Shift+Z"),
            )?;
            let find_item =
                MenuItem::with_id(app, "menu_find", "Find…", true, Some("CmdOrCtrl+F"))?;
            let find_replace_item = MenuItem::with_id(
                app,
                "menu_find_replace",
                "Find and Replace…",
                true,
                Some("CmdOrCtrl+H"),
            )?;

            let edit_menu = Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &undo_item,
                    &redo_item,
                    &PredefinedMenuItem::separator(app)?,
                    &find_item,
                    &find_replace_item,
                ],
            )?;

            let new_slide_item = MenuItem::with_id(
                app,
                "menu_new_slide",
                "New Slide",
                true,
                Some("CmdOrCtrl+Shift+N"),
            )?;
            let text_box_item =
                MenuItem::with_id(app, "menu_text_box", "Text Box", true, None::<&str>)?;
            let image_item =
                MenuItem::with_id(app, "menu_image", "Image…", true, None::<&str>)?;
            let shape_item =
                MenuItem::with_id(app, "menu_shape", "Shape…", true, None::<&str>)?;
            let table_item =
                MenuItem::with_id(app, "menu_table", "Table…", true, None::<&str>)?;
            let chart_item =
                MenuItem::with_id(app, "menu_chart", "Chart…", true, None::<&str>)?;
            let comment_item =
                MenuItem::with_id(app, "menu_comment", "Comment", true, None::<&str>)?;

            let insert_menu = Submenu::with_items(
                app,
                "Insert",
                true,
                &[
                    &new_slide_item,
                    &PredefinedMenuItem::separator(app)?,
                    &text_box_item,
                    &image_item,
                    &shape_item,
                    &table_item,
                    &chart_item,
                    &PredefinedMenuItem::separator(app)?,
                    &comment_item,
                ],
            )?;

            let bold_item =
                MenuItem::with_id(app, "menu_bold", "Bold", true, Some("CmdOrCtrl+B"))?;
            let italic_item =
                MenuItem::with_id(app, "menu_italic", "Italic", true, Some("CmdOrCtrl+I"))?;
            let underline_item =
                MenuItem::with_id(app, "menu_underline", "Underline", true, Some("CmdOrCtrl+U"))?;

            let format_menu = Submenu::with_items(
                app,
                "Format",
                true,
                &[&bold_item, &italic_item, &underline_item],
            )?;

            let present_item = MenuItem::with_id(
                app,
                "menu_present",
                "Start Presentation",
                true,
                Some("CmdOrCtrl+Return"),
            )?;

            let slideshow_menu = Submenu::with_items(
                app,
                "Slide Show",
                true,
                &[&present_item],
            )?;

            let shortcuts_item = MenuItem::with_id(
                app,
                "menu_shortcuts",
                "Keyboard Shortcuts",
                true,
                Some("CmdOrCtrl+?"),
            )?;
            let help_menu = Submenu::with_items(
                app,
                "Help",
                true,
                &[&shortcuts_item],
            )?;

            let menu = Menu::with_items(
                app,
                &[
                    &file_menu,
                    &edit_menu,
                    &insert_menu,
                    &format_menu,
                    &slideshow_menu,
                    &help_menu,
                ],
            )?;
            app.set_menu(menu)?;

            // Load any persisted user-dictionary words into the spell checker
            // before the UI is interactive. Missing file -> empty user dict.
            let state = app.state::<commands::AppState>();
            state.load_user_dictionary();
            Ok(())
        })
        .on_menu_event(|app, event| {
            let _ = app.emit("menu-event", event.id().as_ref());
        })
        .manage(commands::AppState::new())
        .plugin(tauri_plugin_dialog::init())
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
            commands::add_text_box,
            commands::new_slide,
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
            commands::set_build_step_trigger,
            commands::set_build_step_delay,
            commands::set_build_step_motion_path,
            commands::set_slide_reduce_motion,
            commands::set_slide_size,
            commands::set_sections,
            commands::set_rich_notes,
            commands::set_high_contrast,
            commands::set_presenter_settings,
            commands::set_template,
            commands::set_slide_layout,
            commands::set_slide_rehearsed_duration,
            commands::list_templates,
            commands::render_slide_svg,
            commands::export_svg,
            commands::export_png,
            commands::export_pdf,
            commands::compute_morph,
            commands::undo,
            commands::redo,
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
            commands::check_accessibility,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
