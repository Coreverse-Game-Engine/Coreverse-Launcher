use std::{
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use serde::Serialize;

/// The stages of an installation, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallPhase {
    Preparing,
    /// Asking Coreverse DB how to get from the installed version to the new one.
    Planning,
    Downloading,
    /// Copying the installed version (incremental installs only).
    Copying,
    /// Applying delta packages to the copy (incremental installs only).
    Patching,
    Verifying,
    Extracting,
    Finalizing,
}

pub type ProgressCallback = dyn Fn(InstallPhase, u64, u64) + Send + Sync;

const MIN_INTERVAL: Duration = Duration::from_millis(100);

/// Forwards progress to a callback, but not more often than every 100 ms (phase changes and the
/// final tick of a phase always go through) so a fast download does not flood the frontend.
pub struct Reporter {
    callback: Arc<ProgressCallback>,
    last: Mutex<Option<(InstallPhase, Instant)>>,
}

impl Reporter {
    #[must_use]
    pub fn new(callback: Arc<ProgressCallback>) -> Self {
        Self {
            callback,
            last: Mutex::new(None),
        }
    }

    /// Reports unconditionally.
    pub fn phase(&self, phase: InstallPhase, done: u64, total: u64) {
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = Some((phase, Instant::now()));
        (self.callback)(phase, done, total);
    }

    /// Reports unless the previous report of the same phase was less than 100 ms ago.
    pub fn tick(&self, phase: InstallPhase, done: u64, total: u64) {
        let due = {
            let last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
            match *last {
                Some((previous, at)) => {
                    previous != phase
                        || (total > 0 && done >= total)
                        || at.elapsed() >= MIN_INTERVAL
                }
                None => true,
            }
        };
        if due {
            self.phase(phase, done, total);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    fn counting_reporter() -> (Reporter, Arc<AtomicU64>) {
        let count = Arc::new(AtomicU64::new(0));
        let seen = Arc::clone(&count);
        let reporter = Reporter::new(Arc::new(move |_, _, _| {
            seen.fetch_add(1, Ordering::SeqCst);
        }));
        (reporter, count)
    }

    #[test]
    fn rapid_ticks_of_one_phase_are_throttled() {
        let (reporter, count) = counting_reporter();
        for done in 0..50 {
            reporter.tick(InstallPhase::Downloading, done, 100);
        }
        assert!(count.load(Ordering::SeqCst) < 5);
    }

    #[test]
    fn phase_changes_and_completion_always_go_through() {
        let (reporter, count) = counting_reporter();
        reporter.tick(InstallPhase::Downloading, 1, 100);
        reporter.tick(InstallPhase::Downloading, 100, 100);
        reporter.tick(InstallPhase::Verifying, 1, 100);
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }
}
