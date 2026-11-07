use std::{
    fs,
    io::ErrorKind,
    path::Path,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use reqwest::Url;

use super::{
    InstallEvent,
    installer::{self, InstallContext},
    manager::{InstallManager, InstallStatus},
    progress::Reporter,
    store::{self, InstalledEngine},
    version::{EnginePaths, normalize_version},
};
use crate::{Database, Error, Result, error::ErrorInfo};

/// Receives installation events, e.g. to forward them to the frontend.
pub type EventSink = Arc<dyn Fn(InstallEvent) + Send + Sync>;

/// Installs, lists and removes engine versions. This is what the Tauri commands talk to.
pub struct EngineService {
    http: reqwest::Client,
    paths: EnginePaths,
    db: Database,
    installs: Arc<InstallManager>,
    api_base_url: Mutex<Option<String>>,
}

/// Accepts `https://` URLs (and `http://` to localhost in debug builds) and strips a trailing
/// slash.
fn validate_api_base_url(raw: &str) -> Result<String> {
    let invalid = || Error::InvalidUrl(raw.to_owned());
    let url = Url::parse(raw).map_err(|_| invalid())?;
    let local_dev = cfg!(debug_assertions)
        && url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1"));
    if url.scheme() != "https" && !local_dev {
        return Err(invalid());
    }
    if url.host_str().is_none() || !url.username().is_empty() || url.query().is_some() {
        return Err(invalid());
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

impl EngineService {
    pub fn new(data_dir: &Path, db: Database) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(format!("coreverse-launcher/{}", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .https_only(!cfg!(debug_assertions))
            .build()?;
        Ok(Self {
            http,
            paths: EnginePaths::new(data_dir),
            db,
            installs: Arc::new(InstallManager::default()),
            api_base_url: Mutex::new(None),
        })
    }

    /// Tells the service where Coreverse DB's API lives. It can be set once; later calls must
    /// repeat the same value, so a compromised page cannot redirect installs elsewhere.
    pub fn configure_api(&self, raw: &str) -> Result<()> {
        let url = validate_api_base_url(raw)?;
        let mut current = self
            .api_base_url
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match current.as_deref() {
            Some(existing) if existing != url => Err(Error::InvalidUrl(format!(
                "{raw} (already configured with a different address)"
            ))),
            _ => {
                *current = Some(url);
                Ok(())
            }
        }
    }

    fn api_base_url(&self) -> Result<String> {
        self.api_base_url
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or(Error::NotConfigured)
    }

    /// Installed engines, newest first. Rows whose folder has disappeared are dropped.
    pub fn list(&self) -> Result<Vec<InstalledEngine>> {
        let mut present = Vec::new();
        for engine in store::list(&self.db)? {
            if self.paths.install_dir(&engine.version).is_dir() {
                present.push(engine);
            } else {
                store::delete(&self.db, &engine.version)?;
            }
        }
        Ok(present)
    }

    #[must_use]
    pub fn status(&self) -> Option<InstallStatus> {
        self.installs.status()
    }

    /// Asks the running installation to stop. Returns `false` if none is running.
    #[must_use]
    pub fn cancel(&self) -> bool {
        self.installs.cancel()
    }

    pub fn uninstall(&self, requested: &str) -> Result<()> {
        let version = normalize_version(requested)?;
        if self.installs.is_installing(&version) {
            return Err(Error::InstallInProgress);
        }
        if store::get(&self.db, &version)?.is_none() {
            return Err(Error::NotInstalled(version));
        }
        match fs::remove_dir_all(self.paths.install_dir(&version)) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        store::delete(&self.db, &version)
    }

    /// Downloads, verifies and installs `requested`. Progress and the outcome are also reported
    /// through `events`.
    pub async fn install(&self, requested: &str, events: EventSink) -> Result<InstalledEngine> {
        let version = normalize_version(requested)?;
        let api_base_url = self.api_base_url()?;
        let cancel = self.installs.begin(&version)?;

        let reporter = {
            let installs = Arc::clone(&self.installs);
            let events = Arc::clone(&events);
            let version = version.clone();
            Arc::new(Reporter::new(Arc::new(move |phase, done, total| {
                installs.update(phase, done, total);
                events(InstallEvent::Progress {
                    version: version.clone(),
                    phase,
                    done,
                    total,
                });
            })))
        };

        let ctx = InstallContext {
            http: self.http.clone(),
            api_base_url,
            paths: self.paths.clone(),
            db: self.db.clone(),
        };
        let result = installer::install(&ctx, &version, &cancel, &reporter).await;
        self.installs.finish();

        match &result {
            Ok(_) => events(InstallEvent::Finished { version }),
            Err(Error::Cancelled) => events(InstallEvent::Cancelled { version }),
            Err(error) => events(InstallEvent::Failed {
                version,
                error: ErrorInfo::from(error),
            }),
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service(dir: &Path) -> EngineService {
        EngineService::new(dir, Database::open_in_memory().unwrap()).unwrap()
    }

    #[test]
    fn api_base_url_is_validated_and_normalised() {
        assert_eq!(
            validate_api_base_url("https://abc.supabase.co/functions/v1/").unwrap(),
            "https://abc.supabase.co/functions/v1"
        );
        assert!(validate_api_base_url("http://evil.example/functions/v1").is_err());
        assert!(validate_api_base_url("https://u:p@host/").is_err());
        assert!(validate_api_base_url("not a url").is_err());
    }

    #[test]
    fn api_base_url_can_only_be_set_once() {
        let dir = tempfile::tempdir().unwrap();
        let service = service(dir.path());
        assert!(matches!(service.api_base_url(), Err(Error::NotConfigured)));
        service.configure_api("https://a.supabase.co/functions/v1").unwrap();
        service.configure_api("https://a.supabase.co/functions/v1").unwrap();
        assert!(service.configure_api("https://b.supabase.co/functions/v1").is_err());
    }

    #[test]
    fn list_drops_rows_whose_folder_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let service = service(dir.path());
        let record = |version: &str| InstalledEngine {
            version: version.to_owned(),
            status: "stable".to_owned(),
            os: "linux".to_owned(),
            architecture: "x86_64".to_owned(),
            sha256: "a".repeat(64),
            size_bytes: 1,
            installed_at: String::new(),
        };
        store::insert(&service.db, &record("1.0.0")).unwrap();
        store::insert(&service.db, &record("2.0.0")).unwrap();
        fs::create_dir_all(service.paths.install_dir("2.0.0")).unwrap();

        let listed = service.list().unwrap();

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].version, "2.0.0");
        assert!(store::get(&service.db, "1.0.0").unwrap().is_none());
    }

    #[test]
    fn uninstall_removes_files_and_row() {
        let dir = tempfile::tempdir().unwrap();
        let service = service(dir.path());
        let engine = InstalledEngine {
            version: "1.0.0".to_owned(),
            status: "stable".to_owned(),
            os: "linux".to_owned(),
            architecture: "x86_64".to_owned(),
            sha256: "a".repeat(64),
            size_bytes: 1,
            installed_at: String::new(),
        };
        store::insert(&service.db, &engine).unwrap();
        fs::create_dir_all(service.paths.install_dir("1.0.0")).unwrap();

        service.uninstall("1.0.0").unwrap();

        assert!(!service.paths.install_dir("1.0.0").exists());
        assert!(matches!(service.uninstall("1.0.0"), Err(Error::NotInstalled(_))));
        assert!(matches!(service.uninstall("../1.0.0"), Err(Error::InvalidVersion(_))));
    }
}
