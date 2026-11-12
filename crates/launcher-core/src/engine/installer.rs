use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use super::{
    InstallOptions, delta,
    download::{self, ProgressSpan},
    extract::{self, ArchiveFormat},
    metadata::{self, ArtifactMetadata},
    progress::{InstallPhase, Reporter},
    store::{self, InstalledEngine},
    verify,
    version::{EnginePaths, normalize_version},
};
use crate::{Database, Error, Result, platform::Platform};

/// Everything one installation needs.
pub struct InstallContext {
    pub http: reqwest::Client,
    pub api_base_url: String,
    pub paths: EnginePaths,
    pub db: Database,
}

fn join_error(error: tokio::task::JoinError) -> Error {
    Error::Io(std::io::Error::other(error))
}

fn check_cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

/// Installs `requested` for the current platform and returns the stored record.
///
/// When an older version of the engine is already installed the launcher first tries an
/// incremental update (copy that version and apply small delta packages, see [`delta`]); if
/// Coreverse DB has no suitable plan, or anything about the update fails, it downloads the full
/// build as usual. With `options.delete_previous` the older version is removed after a
/// successful install, whichever way the new one was obtained.
///
/// A cancelled or failed install leaves no half-unpacked folder behind. The downloaded archive
/// stays on disk after a cancellation or a network failure so the next attempt can resume it;
/// it is deleted after success and after a checksum mismatch.
pub async fn install(
    ctx: &InstallContext,
    requested: &str,
    options: InstallOptions,
    cancel: &Arc<AtomicBool>,
    reporter: &Arc<Reporter>,
) -> Result<InstalledEngine> {
    let version = normalize_version(requested)?;
    if store::get(&ctx.db, &version)?.is_some() {
        return Err(Error::AlreadyInstalled(version));
    }
    let platform = Platform::current().ok_or_else(|| {
        Error::UnsupportedPlatform(format!(
            "{} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ))
    })?;

    // The newest older version that is installed: where an incremental update starts, and what
    // "delete the old version" removes.
    let previous = delta::choose_base(&store::list(&ctx.db)?, &version, platform, |installed| {
        ctx.paths.install_dir(installed).is_dir()
    });

    reporter.phase(InstallPhase::Preparing, 0, 0);
    let release = metadata::fetch_release(&ctx.http, &ctx.api_base_url, &version).await?;
    check_cancelled(cancel)?;
    let artifact = metadata::select_artifact(&release, platform)?.clone();
    let validated = metadata::validate_artifact(&artifact)?;
    let part = ctx.paths.part_file(&version, platform, &artifact.sha256);

    let delta_tree = match &previous {
        Some(base) => try_delta(ctx, base, &version, platform, cancel, reporter).await?,
        None => None,
    };
    if let Some(tree) = delta_tree {
        let job = DeltaJob {
            paths: ctx.paths.clone(),
            db: ctx.db.clone(),
            version,
            status: release.status,
            artifact,
            platform,
            tree,
            reporter: Arc::clone(reporter),
        };
        let installed = tokio::task::spawn_blocking(move || register_delta(&job))
            .await
            .map_err(join_error)??;
        return Ok(finish(ctx, installed, previous.as_ref(), options).await);
    }

    download::download(
        &ctx.http,
        &validated.url,
        &part,
        artifact.size_bytes,
        cancel,
        reporter,
        ProgressSpan {
            base: 0,
            total: artifact.size_bytes,
        },
    )
    .await?;

    // Verifying the checksum is what ties the file to what Coreverse DB published.
    let actual = {
        let part = part.clone();
        let cancel = Arc::clone(cancel);
        let reporter = Arc::clone(reporter);
        let total = artifact.size_bytes;
        tokio::task::spawn_blocking(move || {
            verify::sha256_file(&part, &cancel, |done| {
                reporter.tick(InstallPhase::Verifying, done, total);
            })
        })
        .await
        .map_err(join_error)??
    };
    if actual != artifact.sha256 {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(Error::ChecksumMismatch);
    }

    let job = UnpackJob {
        paths: ctx.paths.clone(),
        db: ctx.db.clone(),
        version,
        status: release.status,
        artifact,
        platform,
        format: validated.format,
        part,
        cancel: Arc::clone(cancel),
        reporter: Arc::clone(reporter),
    };
    let installed = tokio::task::spawn_blocking(move || unpack_and_register(&job))
        .await
        .map_err(join_error)??;
    Ok(finish(ctx, installed, previous.as_ref(), options).await)
}

/// Attempts an incremental update from `base`. A failure is not an error of the install: it is
/// logged and the caller downloads the full build. Only cancellation ends the install.
async fn try_delta(
    ctx: &InstallContext,
    base: &InstalledEngine,
    version: &str,
    platform: Platform,
    cancel: &Arc<AtomicBool>,
    reporter: &Arc<Reporter>,
) -> Result<Option<delta::BuiltTree>> {
    let request = delta::DeltaRequest {
        http: &ctx.http,
        api_base_url: &ctx.api_base_url,
        paths: &ctx.paths,
        platform,
        base,
        target: version,
    };
    match delta::build(&request, cancel, reporter).await {
        Ok(tree) => Ok(tree),
        Err(Error::Cancelled) => Err(Error::Cancelled),
        Err(error) => {
            tracing::warn!(
                %error,
                from = %base.version,
                to = %version,
                "the incremental update failed, downloading the full build instead"
            );
            Ok(None)
        }
    }
}

/// Runs after a successful install: removes the previous version when the user asked for it. A
/// failure to remove it never fails the install.
async fn finish(
    ctx: &InstallContext,
    installed: InstalledEngine,
    previous: Option<&InstalledEngine>,
    options: InstallOptions,
) -> InstalledEngine {
    if let (true, Some(previous)) = (options.delete_previous, previous) {
        let paths = ctx.paths.clone();
        let db = ctx.db.clone();
        let version = previous.version.clone();
        let removed =
            tokio::task::spawn_blocking(move || remove_previous(&paths, &db, &version)).await;
        if let Err(error) = removed {
            tracing::warn!(%error, "removing the previous engine version was interrupted");
        }
    }
    installed
}

fn remove_previous(paths: &EnginePaths, db: &Database, version: &str) {
    match fs::remove_dir_all(paths.install_dir(version)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            tracing::warn!(%error, %version, "could not remove the previous engine version");
            return;
        }
    }
    if let Err(error) = store::delete(db, version) {
        tracing::warn!(%error, %version, "could not forget the previous engine version");
    }
}

struct DeltaJob {
    paths: EnginePaths,
    db: Database,
    version: String,
    status: String,
    artifact: ArtifactMetadata,
    platform: Platform,
    tree: delta::BuiltTree,
    reporter: Arc<Reporter>,
}

fn register_delta(job: &DeltaJob) -> Result<InstalledEngine> {
    job.reporter.phase(InstallPhase::Finalizing, 0, 0);
    let final_dir = job.paths.install_dir(&job.version);
    replace_dir(&job.tree.root(), &final_dir)?;
    // The record carries the full build's hash and size, like a normal install.
    record_install(
        &job.db,
        &final_dir,
        &InstalledEngine {
            version: job.version.clone(),
            status: job.status.clone(),
            os: job.platform.os.to_owned(),
            architecture: job.platform.architecture.to_owned(),
            sha256: job.artifact.sha256.clone(),
            size_bytes: job.artifact.size_bytes,
            installed_at: String::new(),
        },
    )
}

/// Stores the record of a folder that is already in place; takes the folder away again when
/// storing fails so no engine exists without a record.
fn record_install(
    db: &Database,
    final_dir: &Path,
    record: &InstalledEngine,
) -> Result<InstalledEngine> {
    match store::insert(db, record) {
        Ok(stored) => Ok(stored),
        Err(error) => {
            let _ = fs::remove_dir_all(final_dir);
            Err(error)
        }
    }
}

struct UnpackJob {
    paths: EnginePaths,
    db: Database,
    version: String,
    status: String,
    artifact: ArtifactMetadata,
    platform: Platform,
    format: ArchiveFormat,
    part: PathBuf,
    cancel: Arc<AtomicBool>,
    reporter: Arc<Reporter>,
}

fn unpack_and_register(job: &UnpackJob) -> Result<InstalledEngine> {
    let staging_root = job.paths.staging_root();
    fs::create_dir_all(&staging_root)?;
    // Removed again on every exit path, including errors and cancellation.
    let staging = tempfile::Builder::new()
        .prefix(&format!("{}-", job.version))
        .tempdir_in(&staging_root)?;

    extract::extract(
        &job.part,
        job.format,
        staging.path(),
        &job.cancel,
        &mut |done, total| job.reporter.tick(InstallPhase::Extracting, done, total),
    )?;
    check_cancelled(&job.cancel)?;

    job.reporter.phase(InstallPhase::Finalizing, 0, 0);
    let content_root = extract::content_root(staging.path())?;
    let final_dir = job.paths.install_dir(&job.version);
    replace_dir(&content_root, &final_dir)?;

    let record = InstalledEngine {
        version: job.version.clone(),
        status: job.status.clone(),
        os: job.platform.os.to_owned(),
        architecture: job.platform.architecture.to_owned(),
        sha256: job.artifact.sha256.clone(),
        size_bytes: job.artifact.size_bytes,
        installed_at: String::new(),
    };
    let stored = record_install(&job.db, &final_dir, &record)?;
    let _ = fs::remove_file(&job.part);
    Ok(stored)
}

/// Moves `source` to `destination`, replacing a leftover folder of a crashed earlier attempt.
fn replace_dir(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(source, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn unpacks_registers_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("engine.part");
        let mut writer = zip::ZipWriter::new(fs::File::create(&part).unwrap());
        writer
            .start_file("Engine/run", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"bin").unwrap();
        writer.finish().unwrap();

        let db = Database::open_in_memory().unwrap();
        let job = UnpackJob {
            paths: EnginePaths::new(dir.path()),
            db: db.clone(),
            version: "1.0.0".to_owned(),
            status: "stable".to_owned(),
            artifact: ArtifactMetadata {
                os: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
                download_url: "https://h/e.zip".to_owned(),
                sha256: "a".repeat(64),
                size_bytes: 3,
            },
            platform: Platform::from_rust_consts("linux", "x86_64").unwrap(),
            format: ArchiveFormat::Zip,
            part: part.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
            reporter: Arc::new(Reporter::new(Arc::new(|_, _, _| {}))),
        };

        let engine = unpack_and_register(&job).unwrap();

        assert_eq!(engine.version, "1.0.0");
        assert_eq!(
            fs::read(dir.path().join("engines/1.0.0/run")).unwrap(),
            b"bin"
        );
        assert!(
            !part.exists(),
            "the archive is removed after a successful install"
        );
        assert_eq!(store::list(&db).unwrap().len(), 1);
        let leftovers = fs::read_dir(dir.path().join("engines/.staging"))
            .unwrap()
            .count();
        assert_eq!(leftovers, 0, "staging folders are removed");
    }

    #[test]
    fn a_failed_unpack_leaves_no_engine_behind() {
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("engine.part");
        fs::write(&part, b"this is not a zip file").unwrap();
        let db = Database::open_in_memory().unwrap();
        let job = UnpackJob {
            paths: EnginePaths::new(dir.path()),
            db: db.clone(),
            version: "1.0.0".to_owned(),
            status: "stable".to_owned(),
            artifact: ArtifactMetadata {
                os: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
                download_url: "https://h/e.zip".to_owned(),
                sha256: "a".repeat(64),
                size_bytes: 22,
            },
            platform: Platform::from_rust_consts("linux", "x86_64").unwrap(),
            format: ArchiveFormat::Zip,
            part: part.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
            reporter: Arc::new(Reporter::new(Arc::new(|_, _, _| {}))),
        };

        assert!(unpack_and_register(&job).is_err());
        assert!(!dir.path().join("engines/1.0.0").exists());
        assert_eq!(store::list(&db).unwrap(), Vec::<InstalledEngine>::new());
        assert!(part.exists(), "the archive stays for inspection or a retry");
    }
}
