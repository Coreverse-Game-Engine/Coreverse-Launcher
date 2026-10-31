mod commands;
mod events;
mod state;

use launcher_core::{Database, logging};
use tauri::Manager;
#[cfg(any(windows, target_os = "linux"))]
use tauri_plugin_deep_link::DeepLinkExt;

use crate::state::AppState;

/// Brings the main window to the front, e.g. when a second launch is forwarded to us.
fn focus_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.unminimize().and_then(|()| window.set_focus()) {
        tracing::warn!(%error, "failed to focus the main window");
    }
}

/// Starts the Tauri application.
///
/// # Panics
///
/// Panics if the Tauri runtime cannot start.
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin: a second launch (for example the browser opening a
        // `coreverse-launcher://` link on Windows/Linux) is forwarded to the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let log_guard = logging::init(&app.path().app_log_dir()?)?;

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = Database::open(&data_dir.join("launcher.db"))?;

            // Installed packages register the URL scheme themselves; doing it at runtime also
            // covers dev runs, AppImage and portable builds. macOS only supports bundled apps.
            #[cfg(any(windows, target_os = "linux"))]
            app.deep_link().register_all()?;

            tracing::info!(version = env!("CARGO_PKG_VERSION"), "Coreverse Launcher started");
            app.manage(AppState::new(db, log_guard));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_settings,
            commands::update_settings,
            commands::load_refresh_token,
            commands::save_refresh_token,
            commands::clear_refresh_token,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coreverse Launcher");
}
