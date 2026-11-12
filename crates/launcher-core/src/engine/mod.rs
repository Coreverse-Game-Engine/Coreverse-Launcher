//! Installing and managing Coreverse Engine versions.
//!
//! The flow for one install: fetch the release metadata straight from Coreverse DB (so the URL
//! and the SHA-256 never pass through the webview), download the archive (resuming a partial
//! download if there is one), verify its SHA-256 against the DB's value, unpack it into a
//! staging folder and move that into `engines/<version>`.
//!
//! When an older version is already installed the launcher first tries an incremental update
//! (see `delta`): it copies that version and applies small delta packages from Coreverse DB
//! instead of downloading the whole build, and falls back to the normal flow if anything about
//! the incremental update does not work.

mod delta;
mod download;
mod extract;
mod installer;
mod manager;
mod metadata;
mod progress;
mod service;
mod store;
mod verify;
mod version;

use serde::Serialize;

/// Choices the user makes before an installation starts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InstallOptions {
    /// Remove the newest older version of the engine once the new one is installed. Off by
    /// default: normally every installed version is kept side by side.
    pub delete_previous: bool,
}

pub use manager::InstallStatus;
pub use progress::InstallPhase;
pub use service::{EngineService, EventSink};
pub use store::InstalledEngine;

use crate::error::ErrorInfo;

/// Sent to the frontend while an installation runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InstallEvent {
    Progress {
        version: String,
        phase: InstallPhase,
        done: u64,
        total: u64,
    },
    Finished {
        version: String,
    },
    Cancelled {
        version: String,
    },
    Failed {
        version: String,
        error: ErrorInfo,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_serialize_with_a_type_tag() {
        let event = InstallEvent::Progress {
            version: "1.0.0".to_owned(),
            phase: InstallPhase::Downloading,
            done: 5,
            total: 10,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "progress");
        assert_eq!(json["phase"], "downloading");
        assert_eq!(json["done"], 5);
    }
}
