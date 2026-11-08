//! Tauri commands: the thin IPC layer over `launcher-core`.

use std::sync::Arc;

use launcher_core::{
    AppInfo, Error, EventSink, InstallEvent, InstallInfo, InstallStatus, InstalledEngine, Platform,
    Settings, SettingsPatch, settings,
};
use tauri::{AppHandle, Emitter, State};

use crate::{events, state::AppState};

/// Returns basic application information to the frontend.
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo::new(env!("CARGO_PKG_VERSION"))
}

/// How this copy of the launcher was installed and whether the in-app updater may replace it.
#[tauri::command]
pub fn install_info() -> InstallInfo {
    InstallInfo::detect()
}

/// Returns all settings.
#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, Error> {
    settings::load(&state.db)
        .inspect_err(|error| tracing::error!(%error, "failed to load settings"))
}

/// Applies a partial settings update, then notifies the frontend.
#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<Settings, Error> {
    let updated = settings::update(&state.db, patch)
        .inspect_err(|error| tracing::error!(%error, "failed to update settings"))?;
    if let Err(error) = app.emit(events::SETTINGS_CHANGED, updated) {
        tracing::warn!(%error, "failed to emit the settings-changed event");
    }
    Ok(updated)
}

/// Returns the saved refresh token, if any.
#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn load_refresh_token(state: State<'_, AppState>) -> Result<Option<String>, Error> {
    state
        .refresh_tokens
        .load()
        .inspect_err(|error| tracing::warn!(%error, "failed to read the refresh token"))
}

/// Saves the refresh token to the system keyring.
#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn save_refresh_token(state: State<'_, AppState>, token: String) -> Result<(), Error> {
    state
        .refresh_tokens
        .save(&token)
        .inspect_err(|error| tracing::warn!(%error, "failed to save the refresh token"))
}

/// Removes the saved refresh token (sign-out).
#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn clear_refresh_token(state: State<'_, AppState>) -> Result<(), Error> {
    state
        .refresh_tokens
        .clear()
        .inspect_err(|error| tracing::warn!(%error, "failed to clear the refresh token"))
}

/// Tells the launcher where Coreverse DB's API lives. Needed once per run, before installing.
#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn configure_api(state: State<'_, AppState>, base_url: String) -> Result<(), Error> {
    state
        .engines
        .configure_api(&base_url)
        .inspect_err(|error| tracing::warn!(%error, "rejected the API address"))
}

/// The platform name Coreverse DB uses for this machine, e.g. `windows` / `x86_64`.
#[tauri::command]
pub fn current_platform() -> Option<Platform> {
    Platform::current()
}

/// Engine versions installed on this machine, newest first.
#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn list_installed_engines(state: State<'_, AppState>) -> Result<Vec<InstalledEngine>, Error> {
    state
        .engines
        .list()
        .inspect_err(|error| tracing::error!(%error, "failed to list installed engines"))
}

/// Snapshot of the running installation, if any.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn get_install_status(state: State<'_, AppState>) -> Option<InstallStatus> {
    state.engines.status()
}

/// Asks the running installation to stop. Returns `false` if none is running.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn cancel_install(state: State<'_, AppState>) -> bool {
    state.engines.cancel()
}

/// Removes an installed engine version.
#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn uninstall_engine(state: State<'_, AppState>, version: String) -> Result<(), Error> {
    state
        .engines
        .uninstall(&version)
        .inspect_err(|error| tracing::error!(%error, version, "failed to uninstall the engine"))
}

/// Downloads, verifies and installs an engine version. Progress arrives as
/// `engine-install-event` events; the call returns when the installation is over.
#[tauri::command]
pub async fn install_engine(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
) -> Result<InstalledEngine, Error> {
    let events: EventSink = Arc::new(move |event: InstallEvent| {
        if let Err(error) = app.emit(events::ENGINE_INSTALL, &event) {
            tracing::warn!(%error, "failed to emit an install event");
        }
    });
    tracing::info!(version, "installing engine");
    state
        .engines
        .install(&version, events)
        .await
        .inspect(|engine| tracing::info!(version = %engine.version, "engine installed"))
        .inspect_err(|error| tracing::warn!(%error, version, "engine installation did not finish"))
}
