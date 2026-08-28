//! The Tauri shell.
//!
//! This crate owns the window, the plugins and the command surface. It contains
//! no PDF logic at all: every command forwards to `npdf-core`, which is the same
//! code on all five targets.

mod commands;
mod pdfium_setup;
mod platform;
mod state;

use tauri::Manager;

use state::AppState;

/// Entry point for desktop and, through the attribute, for iOS and Android.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init());

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_window_state::Builder::default().build());

    builder
        .setup(|app| {
            // Point the renderer at the PDFium library that ships with the app
            // before anything can try to render.
            pdfium_setup::configure(app);
            app.manage(AppState::new(app)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::core_info,
            commands::open_document_path,
            commands::open_document_bytes,
            commands::close_document,
            commands::list_documents,
            commands::document_summary,
            commands::page_text,
            commands::apply_edit,
            commands::undo,
            commands::redo,
            commands::render_page,
            commands::save_document,
            commands::save_document_bytes,
            commands::release_memory,
            commands::restore_memory_budget,
        ])
        .run(tauri::generate_context!())
        .expect("NPDF konnte nicht gestartet werden");
}
