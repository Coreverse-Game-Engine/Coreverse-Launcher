use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;

use super::progress::InstallPhase;
use crate::{Error, Result};

/// A snapshot of the running installation, so a freshly loaded UI can pick it up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallStatus {
    pub version: String,
    pub phase: InstallPhase,
    pub done: u64,
    pub total: u64,
}

struct Active {
    cancel: Arc<AtomicBool>,
    status: InstallStatus,
}

/// Allows one installation at a time and lets the UI cancel it.
#[derive(Default)]
pub struct InstallManager {
    active: Mutex<Option<Active>>,
}

impl InstallManager {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Active>> {
        self.active.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Claims the single installation slot and returns the flag that cancels it.
    pub fn begin(&self, version: &str) -> Result<Arc<AtomicBool>> {
        let mut active = self.lock();
        if active.is_some() {
            return Err(Error::InstallInProgress);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *active = Some(Active {
            cancel: Arc::clone(&cancel),
            status: InstallStatus {
                version: version.to_owned(),
                phase: InstallPhase::Preparing,
                done: 0,
                total: 0,
            },
        });
        Ok(cancel)
    }

    pub fn update(&self, phase: InstallPhase, done: u64, total: u64) {
        if let Some(active) = self.lock().as_mut() {
            active.status.phase = phase;
            active.status.done = done;
            active.status.total = total;
        }
    }

    pub fn finish(&self) {
        *self.lock() = None;
    }

    /// Asks the running installation to stop. Returns `false` if none is running.
    pub fn cancel(&self) -> bool {
        match self.lock().as_ref() {
            Some(active) => {
                active.cancel.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    pub fn status(&self) -> Option<InstallStatus> {
        self.lock().as_ref().map(|active| active.status.clone())
    }

    pub fn is_installing(&self, version: &str) -> bool {
        self.lock()
            .as_ref()
            .is_some_and(|active| active.status.version == version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_installation_at_a_time() {
        let manager = InstallManager::default();
        manager.begin("1.0.0").unwrap();
        assert!(matches!(
            manager.begin("2.0.0"),
            Err(Error::InstallInProgress)
        ));
        manager.finish();
        assert!(manager.begin("2.0.0").is_ok());
    }

    #[test]
    fn cancel_sets_the_flag_and_status_follows_updates() {
        let manager = InstallManager::default();
        assert!(!manager.cancel());
        let flag = manager.begin("1.0.0").unwrap();
        manager.update(InstallPhase::Downloading, 5, 10);
        assert_eq!(manager.status().unwrap().done, 5);
        assert!(manager.is_installing("1.0.0"));
        assert!(!manager.is_installing("2.0.0"));
        assert!(manager.cancel());
        assert!(flag.load(Ordering::Relaxed));
    }
}
