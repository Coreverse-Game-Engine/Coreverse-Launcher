//! Building the new version's folder from an installed older one.
//!
//! 1. Copy the installed version into a staging folder.
//! 2. Apply the delta packages in order: bsdiff patches for changed files, whole files for new
//!    or replaced ones, removals. Every old file is checked against the hash the package
//!    expects and every new file against the hash it promises, so a modified or damaged base
//!    never turns into a silently wrong result.
//! 3. Make the folder match the target manifest exactly: drop every file it does not list, check
//!    every listed file (size and SHA-256), create symbolic links, set permissions.
//!
//! Any failure returns an error and the caller falls back to downloading the full build.

use std::{
    collections::HashSet,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool, atomic::Ordering},
};

use sha2::{Digest, Sha256};
use zip::ZipArchive;

use super::{
    bad,
    manifest::{FileKind, Manifest},
    package::{self, DeltaOp},
};
use crate::{
    Error, Result,
    engine::{
        progress::{InstallPhase, Reporter},
        verify,
    },
    platform::Platform,
};

/// Files are patched in memory, so a single file may not be bigger than this. A bigger one makes
/// the update fall back to a full download.
const MAX_IN_MEMORY: u64 = 1 << 30;

/// One delta package on disk, with the versions it connects.
pub struct StepFile {
    pub path: PathBuf,
    pub from: String,
    pub to: String,
}

/// Everything [`build`] needs. All fields are owned so the job can move to a blocking thread.
pub struct BuildJob {
    /// The installed version to start from.
    pub source: PathBuf,
    /// Where the new version is built. Must not exist yet.
    pub tree: PathBuf,
    pub steps: Vec<StepFile>,
    pub manifest: Manifest,
    pub platform: Platform,
    pub cancel: Arc<AtomicBool>,
    pub reporter: Arc<Reporter>,
}

fn check_cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

fn sha256_bytes(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn required<'a>(value: Option<&'a str>, path: &str) -> Result<&'a str> {
    value.ok_or_else(|| bad(format!("{path} is missing a field")))
}

// --- copying -------------------------------------------------------------------------------

fn tree_size(dir: &Path, cancel: &AtomicBool) -> Result<u64> {
    let mut total = 0_u64;
    for entry in fs::read_dir(dir)? {
        check_cancelled(cancel)?;
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            total += tree_size(&entry.path(), cancel)?;
        } else if kind.is_file() {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}

#[cfg(unix)]
fn copy_symlink(from: &Path, to: &Path) -> Result<()> {
    let target = fs::read_link(from)?;
    std::os::unix::fs::symlink(target, to)?;
    Ok(())
}

#[cfg(not(unix))]
fn copy_symlink(_from: &Path, _to: &Path) -> Result<()> {
    Err(bad(
        "symbolic links are not supported on this platform".to_owned()
    ))
}

struct CopyProgress<'a> {
    copied: u64,
    total: u64,
    cancel: &'a AtomicBool,
    reporter: &'a Reporter,
}

fn copy_tree(source: &Path, destination: &Path, progress: &mut CopyProgress<'_>) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        check_cancelled(progress.cancel)?;
        let entry = entry?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&from, &to, progress)?;
        } else if kind.is_symlink() {
            copy_symlink(&from, &to)?;
        } else if kind.is_file() {
            // `fs::copy` keeps the permission bits.
            progress.copied += fs::copy(&from, &to)?;
            progress
                .reporter
                .tick(InstallPhase::Copying, progress.copied, progress.total);
        }
    }
    Ok(())
}

// --- applying ------------------------------------------------------------------------------

/// A write target that refuses to grow past `cap` bytes, so a hostile patch cannot exhaust
/// memory.
struct CappedVec {
    data: Vec<u8>,
    cap: usize,
}

impl Write for CappedVec {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.data.len().saturating_add(buf.len()) > self.cap {
            return Err(io::Error::other("the patched file would be too large"));
        }
        self.data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn bspatch(old: &[u8], patch: &[u8]) -> Result<Vec<u8>> {
    let patcher = qbsdiff::Bspatch::new(patch)
        .map_err(|error| bad(format!("a patch is not valid: {error}")))?;
    let mut out = CappedVec {
        data: Vec::new(),
        cap: usize::try_from(MAX_IN_MEMORY).unwrap_or(usize::MAX),
    };
    patcher
        .apply(old, &mut out)
        .map_err(|error| bad(format!("a patch could not be applied: {error}")))?;
    Ok(out.data)
}

/// Refuses to touch anything below a symbolic link, so a link left over from the old version
/// cannot redirect a write outside the new folder.
fn ensure_no_symlink_ancestors(tree: &Path, rel: &Path) -> Result<()> {
    let Some(parent) = rel.parent() else {
        return Ok(());
    };
    let mut current = tree.to_path_buf();
    for component in parent.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(bad(format!("{} lies below a symbolic link", rel.display())));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn temp_name(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".cvdelta-tmp");
    target.with_file_name(name)
}

/// Writes `data` to `rel` below `tree` through a temporary file, so a crash never leaves a
/// half-written file under the real name.
fn write_file(
    tree: &Path,
    rel: &Path,
    data: &[u8],
    permissions: Option<fs::Permissions>,
) -> Result<()> {
    ensure_no_symlink_ancestors(tree, rel)?;
    let target = tree.join(rel);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::symlink_metadata(&target).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(bad(format!("{} is a folder, not a file", rel.display())));
    }
    let temporary = temp_name(&target);
    fs::write(&temporary, data)?;
    if let Some(permissions) = permissions {
        fs::set_permissions(&temporary, permissions)?;
    }
    fs::rename(&temporary, &target)?;
    Ok(())
}

fn remove_path(tree: &Path, rel: &Path) -> Result<()> {
    ensure_no_symlink_ancestors(tree, rel)?;
    let target = tree.join(rel);
    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(&target)?,
        Ok(_) => fs::remove_file(&target)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

struct ApplyProgress<'a> {
    done: u64,
    total: u64,
    cancel: &'a AtomicBool,
    reporter: &'a Reporter,
}

fn apply_step(
    tree: &Path,
    archive: &mut ZipArchive<std::fs::File>,
    doc: &package::DeltaDoc,
    progress: &mut ApplyProgress<'_>,
) -> Result<()> {
    for entry in &doc.entries {
        check_cancelled(progress.cancel)?;
        match entry.op {
            DeltaOp::Remove => remove_path(tree, &entry.rel)?,
            DeltaOp::Add | DeltaOp::Replace => {
                let blob = package::read_blob(
                    archive,
                    required(entry.blob.as_deref(), &entry.path)?,
                    MAX_IN_MEMORY,
                )?;
                if sha256_bytes(&blob) != required(entry.new_sha256.as_deref(), &entry.path)? {
                    return Err(bad(format!("{} does not match its hash", entry.path)));
                }
                write_file(tree, &entry.rel, &blob, None)?;
            }
            DeltaOp::Patch => {
                ensure_no_symlink_ancestors(tree, &entry.rel)?;
                let target = tree.join(&entry.rel);
                let metadata = fs::symlink_metadata(&target).map_err(|_| {
                    bad(format!(
                        "{} is missing from the installed version",
                        entry.path
                    ))
                })?;
                if !metadata.is_file() || metadata.len() > MAX_IN_MEMORY {
                    return Err(bad(format!("{} cannot be patched", entry.path)));
                }
                let old = fs::read(&target)?;
                if sha256_bytes(&old) != required(entry.old_sha256.as_deref(), &entry.path)? {
                    return Err(bad(format!(
                        "{} differs from the version the patch was made for",
                        entry.path
                    )));
                }
                let patch = package::read_blob(
                    archive,
                    required(entry.blob.as_deref(), &entry.path)?,
                    MAX_IN_MEMORY,
                )?;
                let new = bspatch(&old, &patch)?;
                if sha256_bytes(&new) != required(entry.new_sha256.as_deref(), &entry.path)? {
                    return Err(bad(format!("patching {} gave a wrong result", entry.path)));
                }
                write_file(tree, &entry.rel, &new, Some(metadata.permissions()))?;
            }
        }
        progress.done += 1;
        progress
            .reporter
            .tick(InstallPhase::Patching, progress.done, progress.total);
    }
    Ok(())
}

// --- making the folder match the manifest --------------------------------------------------

fn prune(
    root: &Path,
    rel_dir: &Path,
    keep_files: &HashSet<PathBuf>,
    keep_dirs: &HashSet<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(root.join(rel_dir))? {
        let entry = entry?;
        let rel = rel_dir.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            prune(root, &rel, keep_files, keep_dirs)?;
            if !keep_dirs.contains(&rel) {
                // Only succeeds for an empty folder, which is exactly what is left here.
                let _ = fs::remove_dir(root.join(&rel));
            }
        } else if !keep_files.contains(&rel) {
            fs::remove_file(root.join(&rel))?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn create_symlink(tree: &Path, rel: &Path, target: &str) -> Result<()> {
    let link = tree.join(rel);
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::symlink_metadata(&link) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if fs::read_link(&link)? == Path::new(target) {
                return Ok(());
            }
            fs::remove_file(&link)?;
        }
        Ok(metadata) if metadata.is_dir() => {
            return Err(bad(format!("{} is a folder, not a link", rel.display())));
        }
        Ok(_) => fs::remove_file(&link)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    std::os::unix::fs::symlink(target, &link)?;
    Ok(())
}

#[cfg(not(unix))]
fn create_symlink(_tree: &Path, _rel: &Path, _target: &str) -> Result<()> {
    Err(bad(
        "symbolic links are not supported on this platform".to_owned()
    ))
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o7777))?;
    Ok(())
}

// Permission bits mean nothing here; the signature matches the Unix version so the caller is
// the same on every platform.
#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

fn finalize(
    tree: &Path,
    manifest: &Manifest,
    cancel: &AtomicBool,
    reporter: &Reporter,
) -> Result<()> {
    let keep_files: HashSet<PathBuf> = manifest.files.iter().map(|file| file.rel.clone()).collect();
    let mut keep_dirs: HashSet<PathBuf> = HashSet::new();
    for rel in &keep_files {
        for ancestor in rel.ancestors().skip(1) {
            if !ancestor.as_os_str().is_empty() {
                keep_dirs.insert(ancestor.to_path_buf());
            }
        }
    }
    prune(tree, Path::new(""), &keep_files, &keep_dirs)?;

    let total: u64 = manifest
        .files
        .iter()
        .filter(|file| file.kind == FileKind::File)
        .map(|file| file.size)
        .sum();
    let mut done = 0_u64;
    reporter.phase(InstallPhase::Verifying, 0, total);
    for file in manifest
        .files
        .iter()
        .filter(|file| file.kind == FileKind::File)
    {
        check_cancelled(cancel)?;
        let path = tree.join(&file.rel);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| bad(format!("{} is missing from the result", file.path)))?;
        if !metadata.is_file() || metadata.len() != file.size {
            return Err(bad(format!("{} has the wrong size", file.path)));
        }
        let actual = verify::sha256_file(&path, cancel, |bytes| {
            reporter.tick(InstallPhase::Verifying, done + bytes, total);
        })?;
        if actual != file.sha256 {
            return Err(bad(format!("{} does not match the manifest", file.path)));
        }
        done += file.size;
    }
    reporter.phase(InstallPhase::Verifying, total, total);

    for file in &manifest.files {
        match file.kind {
            FileKind::Symlink => {
                create_symlink(tree, &file.rel, file.target.as_deref().unwrap_or_default())?;
            }
            FileKind::File => {
                if let Some(mode) = file.mode {
                    set_mode(&tree.join(&file.rel), mode)?;
                }
            }
        }
    }
    Ok(())
}

/// Builds the new version in `job.tree`. Blocking: call it from `spawn_blocking`.
pub fn build(job: &BuildJob) -> Result<()> {
    // Open and check every package before the expensive copy starts.
    let mut opened = Vec::with_capacity(job.steps.len());
    let mut total_entries = 0_u64;
    for step in &job.steps {
        let (archive, doc) = package::open(&step.path, &step.from, &step.to, job.platform)?;
        total_entries += u64::try_from(doc.entries.len()).unwrap_or(u64::MAX);
        opened.push((archive, doc));
    }

    job.reporter.phase(InstallPhase::Copying, 0, 0);
    let total = tree_size(&job.source, &job.cancel)?;
    let mut copy = CopyProgress {
        copied: 0,
        total,
        cancel: &job.cancel,
        reporter: &job.reporter,
    };
    copy_tree(&job.source, &job.tree, &mut copy)?;
    job.reporter.phase(InstallPhase::Copying, total, total);

    job.reporter.phase(InstallPhase::Patching, 0, total_entries);
    let mut progress = ApplyProgress {
        done: 0,
        total: total_entries,
        cancel: &job.cancel,
        reporter: &job.reporter,
    };
    for (archive, doc) in &mut opened {
        apply_step(&job.tree, archive, doc, &mut progress)?;
    }
    job.reporter
        .phase(InstallPhase::Patching, total_entries, total_entries);

    finalize(&job.tree, &job.manifest, &job.cancel, &job.reporter)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::engine::delta::package::test_support::write_package;

    fn sha(data: &[u8]) -> String {
        sha256_bytes(data)
    }

    fn patch_between(old: &[u8], new: &[u8]) -> Vec<u8> {
        let mut patch = Vec::new();
        qbsdiff::Bsdiff::new(old, new).compare(&mut patch).unwrap();
        patch
    }

    fn big(seed: u8, len: usize) -> Vec<u8> {
        (0..len)
            .map(|i| {
                seed.wrapping_add(u8::try_from(i % 251).unwrap_or_default())
                    .wrapping_mul(3)
            })
            .collect()
    }

    fn silent_reporter() -> Arc<Reporter> {
        Arc::new(Reporter::new(Arc::new(|_, _, _| {})))
    }

    struct Fixture {
        dir: tempfile::TempDir,
        old_engine: Vec<u8>,
        new_engine: Vec<u8>,
        added: Vec<u8>,
        data: Vec<u8>,
    }

    impl Fixture {
        /// Old version: bin/engine, data/keep.bin, data/old.txt, user/notes.txt (not in any
        /// manifest). New version: patched bin/engine, the same data/keep.bin, a new
        /// data/new.bin, and a link lib/link -> ../bin/engine.
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let old_engine = big(1, 40_000);
            let mut new_engine = old_engine.clone();
            new_engine[1_000..1_050].fill(0xAB);
            new_engine.extend_from_slice(b"tail");
            let added = b"brand new file".to_vec();
            let data = b"unchanged data".to_vec();

            let source = dir.path().join("old");
            fs::create_dir_all(source.join("bin")).unwrap();
            fs::create_dir_all(source.join("data")).unwrap();
            fs::create_dir_all(source.join("user")).unwrap();
            fs::write(source.join("bin/engine"), &old_engine).unwrap();
            fs::write(source.join("data/keep.bin"), &data).unwrap();
            fs::write(source.join("data/old.txt"), b"to be removed").unwrap();
            fs::write(source.join("user/notes.txt"), b"not part of the engine").unwrap();

            Self {
                dir,
                old_engine,
                new_engine,
                added,
                data,
            }
        }

        fn manifest(&self) -> Manifest {
            // Symbolic links need Unix; on other platforms an update with one falls back to the
            // full download, so the fixture only has the link where it can be created.
            let link = if cfg!(unix) {
                r#",{"path":"lib/link","kind":"symlink","target":"../bin/engine"}"#
            } else {
                ""
            };
            let json = format!(
                r#"{{"format":1,"version":"1.1.0","os":"linux","architecture":"x86_64","files":[
                {{"path":"bin/engine","kind":"file","size":{},"sha256":"{}","mode":493}},
                {{"path":"data/keep.bin","kind":"file","size":{},"sha256":"{}"}},
                {{"path":"data/new.bin","kind":"file","size":{},"sha256":"{}"}}{link}]}}"#,
                self.new_engine.len(),
                sha(&self.new_engine),
                self.data.len(),
                sha(&self.data),
                self.added.len(),
                sha(&self.added),
            );
            Manifest::parse(
                json.as_bytes(),
                "1.1.0",
                Platform::from_rust_consts("linux", "x86_64").unwrap(),
            )
            .unwrap()
        }

        fn package(&self, old_hash: &str) -> PathBuf {
            let patch = patch_between(&self.old_engine, &self.new_engine);
            let doc = format!(
                r#"{{"format":1,"algorithm":"bsdiff","from_version":"1.0.0","to_version":"1.1.0",
                "os":"linux","architecture":"x86_64","entries":[
                {{"path":"bin/engine","op":"patch","old_sha256":"{old_hash}","new_sha256":"{}","blob":"blobs/0"}},
                {{"path":"data/new.bin","op":"add","new_sha256":"{}","blob":"blobs/1"}},
                {{"path":"data/old.txt","op":"remove"}}]}}"#,
                sha(&self.new_engine),
                sha(&self.added),
            );
            let archive_path = self.dir.path().join("delta.zip");
            write_package(
                &archive_path,
                &doc,
                &[
                    ("blobs/0", patch.as_slice()),
                    ("blobs/1", self.added.as_slice()),
                ],
            );
            archive_path
        }

        fn job(&self, package: PathBuf, manifest: Manifest) -> BuildJob {
            BuildJob {
                source: self.dir.path().join("old"),
                tree: self.dir.path().join("new"),
                steps: vec![StepFile {
                    path: package,
                    from: "1.0.0".to_owned(),
                    to: "1.1.0".to_owned(),
                }],
                manifest,
                platform: Platform::from_rust_consts("linux", "x86_64").unwrap(),
                cancel: Arc::new(AtomicBool::new(false)),
                reporter: silent_reporter(),
            }
        }
    }

    #[test]
    fn builds_the_new_version_from_the_old_one() {
        let fixture = Fixture::new();
        let package = fixture.package(&sha(&fixture.old_engine));
        let job = fixture.job(package, fixture.manifest());

        build(&job).unwrap();

        let tree = &job.tree;
        assert_eq!(
            fs::read(tree.join("bin/engine")).unwrap(),
            fixture.new_engine
        );
        assert_eq!(fs::read(tree.join("data/keep.bin")).unwrap(), fixture.data);
        assert_eq!(fs::read(tree.join("data/new.bin")).unwrap(), fixture.added);
        assert!(
            !tree.join("data/old.txt").exists(),
            "removed by the package"
        );
        assert!(
            !tree.join("user").exists(),
            "files outside the manifest are dropped"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(tree.join("bin/engine"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
            assert_eq!(
                fs::read_link(tree.join("lib/link")).unwrap(),
                Path::new("../bin/engine")
            );
        }
        // The installed version is not touched.
        assert_eq!(
            fs::read(fixture.dir.path().join("old/bin/engine")).unwrap(),
            fixture.old_engine
        );
    }

    #[test]
    fn a_modified_base_file_fails_the_update() {
        let fixture = Fixture::new();
        let package = fixture.package(&sha(&fixture.old_engine));
        fs::write(
            fixture.dir.path().join("old/bin/engine"),
            b"user edited this",
        )
        .unwrap();
        let job = fixture.job(package, fixture.manifest());

        assert!(matches!(build(&job), Err(Error::Delta(_))));
    }

    #[test]
    fn a_wrong_old_hash_in_the_package_fails_the_update() {
        let fixture = Fixture::new();
        let package = fixture.package(&"0".repeat(64));
        let job = fixture.job(package, fixture.manifest());

        assert!(matches!(build(&job), Err(Error::Delta(_))));
    }

    #[test]
    fn a_result_that_differs_from_the_manifest_fails_the_update() {
        let fixture = Fixture::new();
        let package = fixture.package(&sha(&fixture.old_engine));
        let mut manifest = fixture.manifest();
        // The manifest claims another content for the unchanged file.
        manifest.files[1].sha256 = "f".repeat(64);
        let job = fixture.job(package, manifest);

        assert!(matches!(build(&job), Err(Error::Delta(_))));
    }

    #[test]
    fn cancelling_stops_the_build() {
        let fixture = Fixture::new();
        let package = fixture.package(&sha(&fixture.old_engine));
        let job = fixture.job(package, fixture.manifest());
        job.cancel.store(true, Ordering::Relaxed);

        assert!(matches!(build(&job), Err(Error::Cancelled)));
    }

    #[cfg(unix)]
    #[test]
    fn writes_never_follow_a_symlink_left_over_from_the_old_version() {
        let fixture = Fixture::new();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), fixture.dir.path().join("old/data/escape"))
            .unwrap();
        let tree = fixture.dir.path().join("new");
        fs::create_dir_all(&tree).unwrap();
        std::os::unix::fs::symlink(outside.path(), tree.join("escape")).unwrap();

        assert!(write_file(&tree, Path::new("escape/payload"), b"x", None).is_err());
        assert!(!outside.path().join("payload").exists());
    }
}
