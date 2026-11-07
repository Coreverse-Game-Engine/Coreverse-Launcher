use std::{path::Path, sync::Arc};

use launcher_core::{Database, EngineService, LogGuard, RefreshTokenStore, Result};

/// Shared application state, registered with Tauri's `manage`.
pub struct AppState {
    pub db: Database,
    pub refresh_tokens: RefreshTokenStore,
    pub engines: Arc<EngineService>,
    /// Kept only so file logging stays alive for as long as the app runs.
    _log_guard: LogGuard,
}

impl AppState {
    pub fn new(db: Database, log_guard: LogGuard, data_dir: &Path) -> Result<Self> {
        Ok(Self {
            engines: Arc::new(EngineService::new(data_dir, db.clone())?),
            db,
            refresh_tokens: RefreshTokenStore::system(),
            _log_guard: log_guard,
        })
    }
}
