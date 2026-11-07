//! Names of the events the backend sends to the frontend.
//!
//! The same strings are mirrored in `src/lib/tauri.ts`; keep both sides in sync.

/// Emitted with the full `Settings` payload after every successful settings update.
pub const SETTINGS_CHANGED: &str = "settings-changed";

/// Emitted with an `InstallEvent` payload while an engine installation runs.
pub const ENGINE_INSTALL: &str = "engine-install-event";
