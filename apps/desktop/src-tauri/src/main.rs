#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod document;
mod versions;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    Emitter, Manager,
};

fn main() {
    let _version = slides_core::version();

    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            let application_menu = Submenu::with_items(
                app,
                "900Slides",
                true,
                &[
                    &PredefinedMenuItem::about(
                        app,
                        Some("About 900Slides"),
                        Some(tauri::menu::AboutMetadata {
                            name: Some("900Slides".into()),
                            version: Some(app.package_info().version.to_string()),
                            ..Default::default()
                        }),
                    )?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, Some("Hide 900Slides"))?,
                    &PredefinedMenuItem::hide_others(app, None)?,
                    &PredefinedMenuItem::show_all(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    // Cocoa's predefined Quit invokes terminate: directly.
                    // Route the accelerator through the editor's save guard.
                    &MenuItem::with_id(
                        app,
                        "menu_quit",
                        "Quit 900Slides",
                        true,
                        Some("CmdOrCtrl+Q"),
                    )?,
                ],
            )?;
            let new_item = MenuItem::with_id(app, "menu_new", "New", true, Some("CmdOrCtrl+N"))?;
            let open_item =
                MenuItem::with_id(app, "menu_open", "Open…", true, Some("CmdOrCtrl+O"))?;
            let save_item = MenuItem::with_id(app, "menu_save", "Save", true, Some("CmdOrCtrl+S"))?;
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
                    #[cfg(not(target_os = "macos"))]
                    &PredefinedMenuItem::separator(app)?,
                    #[cfg(not(target_os = "macos"))]
                    &MenuItem::with_id(app, "menu_quit", "Quit", true, Some("Ctrl+Q"))?,
                ],
            )?;

            let undo_item = MenuItem::with_id(app, "menu_undo", "Undo", true, Some("CmdOrCtrl+Z"))?;
            let redo_item =
                MenuItem::with_id(app, "menu_redo", "Redo", true, Some("CmdOrCtrl+Shift+Z"))?;
            let find_item =
                MenuItem::with_id(app, "menu_find", "Find…", true, Some("CmdOrCtrl+F"))?;
            let find_replace_item = MenuItem::with_id(
                app,
                "menu_find_replace",
                "Find and Replace…",
                true,
                Some(if cfg!(target_os = "macos") {
                    "Cmd+Alt+F"
                } else {
                    "Ctrl+H"
                }),
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
            let image_item = MenuItem::with_id(app, "menu_image", "Image…", true, None::<&str>)?;
            let shape_item = MenuItem::with_id(app, "menu_shape", "Shape…", true, None::<&str>)?;
            let table_item = MenuItem::with_id(app, "menu_table", "Table…", true, None::<&str>)?;
            let chart_item = MenuItem::with_id(app, "menu_chart", "Chart…", true, None::<&str>)?;
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

            let bold_item = MenuItem::with_id(app, "menu_bold", "Bold", true, Some("CmdOrCtrl+B"))?;
            let italic_item =
                MenuItem::with_id(app, "menu_italic", "Italic", true, Some("CmdOrCtrl+I"))?;
            let underline_item = MenuItem::with_id(
                app,
                "menu_underline",
                "Underline",
                true,
                Some("CmdOrCtrl+U"),
            )?;

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

            let slideshow_menu = Submenu::with_items(app, "Slide Show", true, &[&present_item])?;

            let shortcuts_item = MenuItem::with_id(
                app,
                "menu_shortcuts",
                "Keyboard Shortcuts",
                true,
                Some("CmdOrCtrl+?"),
            )?;
            let help_menu = Submenu::with_items(app, "Help", true, &[&shortcuts_item])?;

            let menu = Menu::with_items(
                app,
                &[
                    #[cfg(target_os = "macos")]
                    &application_menu,
                    &file_menu,
                    &edit_menu,
                    &insert_menu,
                    &format_menu,
                    &slideshow_menu,
                    &help_menu,
                ],
            )?;
            app.set_menu(menu)?;

            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "menu_quit" {
                // Ask the editor first. Only complete_close may request exit
                // after pending edits and Save/Discard/Cancel are resolved.
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                let _ = app.emit_to("main", "document-close-requested", ());
            } else {
                let _ = app.emit("menu-event", event.id().as_ref());
            }
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.emit("document-close-requested", ());
                }
            }
        })
        .manage(commands::AppState::new())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::new_deck,
            commands::open_deck,
            commands::save_deck,
            commands::get_document_status,
            commands::flush_recovery,
            commands::complete_close,
            commands::get_snapshot,
            commands::edit_text,
            commands::edit_text_box,
            commands::set_run_style,
            commands::set_paragraph_style,
            commands::insert_image,
            commands::add_shape,
            commands::add_text_box,
            commands::new_slide,
            commands::move_slide,
            commands::delete_slide,
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Runtime exit requests retain this additional guard; the exit
            // code does not imply that edits were handled by the editor.
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let confirmed = app
                    .state::<commands::AppState>()
                    .close_confirmed
                    .load(std::sync::atomic::Ordering::SeqCst);
                if !confirmed {
                    api.prevent_exit();
                    let _ = app.emit_to("main", "document-close-requested", ());
                }
            }
        });
}
