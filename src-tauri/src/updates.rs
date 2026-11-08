//! In-app updates, through `tauri-plugin-updater`.
//!
//! Everything runs on the Rust side, so the webview never sees download URLs or signatures:
//! it asks for a check, gets a version and the release notes back, and later asks to install.
//! The plugin verifies the minisign signature of the download against the public key in
//! `tauri.conf.json` before it touches any file.
//!
//! Only installations the launcher owns may update themselves (see
//! [`launcher_core::InstallChannel`]); package managers update the rest.

use std::sync::{Mutex, PoisonError};

use launcher_core::{Error, InstallInfo};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::events;

/// The update the last check found, kept until it is installed.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

impl PendingUpdate {
    fn store(&self, update: Option<Update>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = update;
    }

    fn take(&self) -> Option<Update> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

/// What the frontend learns about an available update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
}

/// Sent to the frontend while an update downloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UpdateEvent {
    Progress { downloaded: u64, total: Option<u64> },
    Installing,
}

fn emit(app: &AppHandle, event: UpdateEvent) {
    if let Err(error) = app.emit(events::UPDATE, event) {
        tracing::warn!(%error, "failed to emit an update event");
    }
}

/// Asks the update server whether a newer version exists. `None` means this one is current.
#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<UpdateInfo>, Error> {
    if !InstallInfo::detect().self_update {
        return Err(Error::UpdateNotSupported);
    }

    let updater = app
        .updater()
        .map_err(|error| Error::Update(error.to_string()))?;
    let found = updater
        .check()
        .await
        .map_err(|error| Error::Update(error.to_string()))?;

    let info = found.as_ref().map(|update| UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
    });
    if let Some(info) = &info {
        tracing::info!(version = %info.version, "an update is available");
    } else {
        tracing::info!("the launcher is up to date");
    }
    pending.store(found);
    Ok(info)
}

/// Downloads, verifies and installs the update the last check found. Progress arrives as
/// `update-event` events. On Windows the installer closes the launcher itself; elsewhere the
/// call returns and the frontend offers a restart.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<(), Error> {
    if !InstallInfo::detect().self_update {
        return Err(Error::UpdateNotSupported);
    }
    let Some(update) = pending.take() else {
        return Err(Error::NoPendingUpdate);
    };

    tracing::info!(version = %update.version, "installing the update");
    let mut downloaded: u64 = 0;
    update
        .download_and_install(
            |chunk, total| {
                downloaded = downloaded.saturating_add(u64::try_from(chunk).unwrap_or(u64::MAX));
                emit(&app, UpdateEvent::Progress { downloaded, total });
            },
            || emit(&app, UpdateEvent::Installing),
        )
        .await
        .inspect_err(|error| tracing::warn!(%error, "the update could not be installed"))
        .map_err(|error| Error::Update(error.to_string()))
}

/// Restarts the launcher, e.g. to start the version that was just installed.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn restart_app(app: AppHandle) {
    app.restart();
}
