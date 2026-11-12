//! Tauri-independent core logic of Coreverse Launcher.
//!
//! Everything that does not need a window or an IPC bridge (SQLite access, settings,
//! logging, and later download and verification) lives in this crate so it can be tested
//! with plain `cargo test`. The Tauri crate only exposes it to the frontend.

mod database;
pub mod engine;
mod error;
pub mod install_channel;
pub mod logging;
mod migrations;
pub mod platform;
pub mod secrets;
pub mod settings;

use serde::Serialize;

pub use database::Database;
pub use engine::{
    EngineService, EventSink, InstallEvent, InstallOptions, InstallPhase, InstallStatus,
    InstalledEngine,
};
pub use error::{Error, ErrorInfo, Result};
pub use install_channel::{InstallChannel, InstallInfo};
pub use logging::LogGuard;
pub use platform::Platform;
pub use secrets::RefreshTokenStore;
pub use settings::{Language, Settings, SettingsPatch, Theme};

/// Display name of the application.
pub const APP_NAME: &str = "Coreverse Launcher";

/// Basic information about the running application, sent to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

impl AppInfo {
    /// Builds the info for the given application version.
    #[must_use]
    pub fn new(version: &str) -> Self {
        Self {
            name: APP_NAME.to_owned(),
            version: version.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_carries_name_and_version() {
        let info = AppInfo::new("1.2.3");
        assert_eq!(info.name, APP_NAME);
        assert_eq!(info.version, "1.2.3");
    }

    #[test]
    fn app_info_serializes_as_camel_case() {
        let json = serde_json::to_string(&AppInfo::new("0.1.0")).unwrap();
        assert_eq!(json, r#"{"name":"Coreverse Launcher","version":"0.1.0"}"#);
    }
}
