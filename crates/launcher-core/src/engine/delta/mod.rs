//! Incremental ("delta") installs: build a new engine version from an installed older one.
//!
//! The normal install downloads the whole build from its published address. Here the launcher
//! asks Coreverse DB for a plan instead ([`plan`]), downloads only a manifest and a few small
//! delta packages from Coreverse DB's storage, copies the installed older version and applies the
//! packages to the copy ([`apply`]). The result is verified file by file against the manifest,
//! whose hash comes from the same answer, so the update is exactly as trustworthy as the full
//! build's checksum.
//!
//! This module never decides to give up on its own: any problem (no plan, damaged file, modified
//! base, full disk) is an error the installer turns into "download the full build instead".

mod apply;
mod manifest;
mod package;
mod plan;

use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use super::{
    download::{self, ProgressSpan},
    progress::{InstallPhase, Reporter},
    store::InstalledEngine,
    verify,
    version::EnginePaths,
};
use crate::{Error, Result, platform::Platform};

fn bad(message: String) -> Error {
    Error::Delta(message)
}

fn join_error(error: tokio::task::JoinError) -> Error {
    Error::Io(std::io::Error::other(error))
}

/// What an incremental update needs to know about the install it is part of.
pub struct DeltaRequest<'a> {
    pub http: &'a reqwest::Client,
    pub api_base_url: &'a str,
    pub paths: &'a EnginePaths,
    pub platform: Platform,
    /// The installed version to start from.
    pub base: &'a InstalledEngine,
    /// The version being installed (normalised).
    pub target: &'a str,
}

/// The finished folder of the new version. Dropping it deletes the staging folder it lives in.
pub struct BuiltTree {
    staging: tempfile::TempDir,
}

impl BuiltTree {
    /// The folder to move into place as the new engine version.
    #[must_use]
    pub fn root(&self) -> PathBuf {
        self.staging.path().join("tree")
    }
}

/// Picks the installed version an incremental update starts from: the newest one that is older
/// than `target`, built for `platform`, and still has its folder on disk.
#[must_use]
pub fn choose_base(
    installed: &[InstalledEngine],
    target: &str,
    platform: Platform,
    folder_exists: impl Fn(&str) -> bool,
) -> Option<InstalledEngine> {
    let target = semver::Version::parse(target).ok()?;
    installed
        .iter()
        .filter(|engine| engine.os == platform.os && engine.architecture == platform.architecture)
        .filter_map(|engine| {
            semver::Version::parse(&engine.version)
                .ok()
                .map(|version| (version, engine))
        })
        .filter(|(version, engine)| *version < target && folder_exists(&engine.version))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, engine)| engine.clone())
}

/// Downloads one planned file (resumable) and checks it against the hash from the plan.
async fn fetch_verified(
    request: &DeltaRequest<'_>,
    item: &plan::Download,
    span: ProgressSpan,
    cancel: &Arc<AtomicBool>,
    reporter: &Arc<Reporter>,
) -> Result<PathBuf> {
    let part = request.paths.delta_file(&item.sha256);
    download::download(
        request.http,
        &item.url,
        &part,
        item.size_bytes,
        cancel,
        reporter,
        span,
    )
    .await?;

    let actual = {
        let part = part.clone();
        let cancel = Arc::clone(cancel);
        tokio::task::spawn_blocking(move || verify::sha256_file(&part, &cancel, |_| {}))
            .await
            .map_err(join_error)??
    };
    if actual != item.sha256 {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(bad(
            "a downloaded file does not match its checksum".to_owned()
        ));
    }
    Ok(part)
}

/// Tries to build `request.target` from `request.base`.
///
/// * `Ok(Some(tree))`: the new version is complete and verified.
/// * `Ok(None)`: Coreverse DB advises a full download (no chain, too large, no manifest, ...).
/// * `Err(_)`: it was attempted and failed. The caller downloads the full build instead, except
///   for [`Error::Cancelled`], which ends the install.
pub async fn build(
    request: &DeltaRequest<'_>,
    cancel: &Arc<AtomicBool>,
    reporter: &Arc<Reporter>,
) -> Result<Option<BuiltTree>> {
    reporter.phase(InstallPhase::Planning, 0, 0);
    let Some(answer) = plan::fetch(
        request.http,
        request.api_base_url,
        &request.base.version,
        request.target,
        request.platform,
    )
    .await?
    else {
        return Ok(None);
    };
    if answer.recommended != "delta" {
        tracing::info!(reason = %answer.reason, "a full download is recommended");
        return Ok(None);
    }
    let validated = answer.validate(&request.base.version, request.target)?;
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }

    // The manifest and every package download share one progress bar.
    let total = validated
        .steps
        .iter()
        .fold(validated.manifest.size_bytes, |sum, step| {
            sum.saturating_add(step.download.size_bytes)
        });
    reporter.phase(InstallPhase::Downloading, 0, total);

    let mut base = 0_u64;
    let manifest_file = fetch_verified(
        request,
        &validated.manifest,
        ProgressSpan { base, total },
        cancel,
        reporter,
    )
    .await?;
    base += validated.manifest.size_bytes;

    let mut files = Vec::with_capacity(validated.steps.len());
    let mut downloaded = vec![manifest_file.clone()];
    for step in &validated.steps {
        let path = fetch_verified(
            request,
            &step.download,
            ProgressSpan { base, total },
            cancel,
            reporter,
        )
        .await?;
        base += step.download.size_bytes;
        downloaded.push(path.clone());
        files.push(apply::StepFile {
            path,
            from: step.from.clone(),
            to: step.to.clone(),
        });
    }

    let manifest_bytes = tokio::fs::read(&manifest_file).await?;
    let manifest = manifest::Manifest::parse(&manifest_bytes, request.target, request.platform)?;

    let staging_root = request.paths.staging_root();
    tokio::fs::create_dir_all(&staging_root).await?;
    // Removed on every exit path, including errors and cancellation.
    let staging = tempfile::Builder::new()
        .prefix(&format!("{}-delta-", request.target))
        .tempdir_in(&staging_root)?;
    let tree = BuiltTree { staging };

    let job = apply::BuildJob {
        source: request.paths.install_dir(&request.base.version),
        tree: tree.root(),
        steps: files,
        manifest,
        platform: request.platform,
        cancel: Arc::clone(cancel),
        reporter: Arc::clone(reporter),
    };
    tokio::task::spawn_blocking(move || apply::build(&job))
        .await
        .map_err(join_error)??;

    for path in downloaded {
        let _ = tokio::fs::remove_file(path).await;
    }
    Ok(Some(tree))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(version: &str, os: &str, arch: &str) -> InstalledEngine {
        InstalledEngine {
            version: version.to_owned(),
            status: "stable".to_owned(),
            os: os.to_owned(),
            architecture: arch.to_owned(),
            sha256: "a".repeat(64),
            size_bytes: 1,
            installed_at: String::new(),
        }
    }

    fn linux() -> Platform {
        Platform::from_rust_consts("linux", "x86_64").unwrap()
    }

    #[test]
    fn the_base_is_the_newest_older_version_of_this_platform() {
        let installed = vec![
            engine("1.0.0", "linux", "x86_64"),
            engine("1.4.0", "linux", "x86_64"),
            engine("1.5.0", "macos", "aarch64"),
            engine("2.0.0", "linux", "x86_64"),
            engine("1.10.0", "linux", "x86_64"),
        ];
        let base = choose_base(&installed, "1.12.0", linux(), |_| true).unwrap();
        assert_eq!(
            base.version, "1.10.0",
            "numeric, not alphabetical, ordering"
        );
    }

    #[test]
    fn versions_that_are_not_older_are_never_a_base() {
        let installed = vec![engine("2.0.0", "linux", "x86_64")];
        assert!(choose_base(&installed, "2.0.0", linux(), |_| true).is_none());
        assert!(choose_base(&installed, "1.0.0", linux(), |_| true).is_none());
    }

    #[test]
    fn a_version_without_its_folder_is_skipped() {
        let installed = vec![
            engine("1.0.0", "linux", "x86_64"),
            engine("1.5.0", "linux", "x86_64"),
        ];
        let base = choose_base(&installed, "2.0.0", linux(), |version| version != "1.5.0").unwrap();
        assert_eq!(base.version, "1.0.0");
    }
}
