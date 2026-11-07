//! Safe unpacking of `.zip` and `.tar.gz` engine archives.
//!
//! The archive is untrusted input (it only passed a checksum, which proves it is the file the
//! DB describes, not that the file is harmless), so every entry is checked: no absolute or
//! `..` paths, no writing through symbolic links, no hard links or device files, symbolic links
//! only when they point inside the install folder, and a cap on the total unpacked size.

use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use crate::{Error, Result};

/// Upper bound for the unpacked size, a guard against zip bombs.
const MAX_EXTRACTED_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_LINK_TARGET_BYTES: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    TarGz,
}

impl ArchiveFormat {
    /// Recognises the format from a file name or URL path.
    #[must_use]
    pub fn from_path(path: &str) -> Option<Self> {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".zip") {
            Some(Self::Zip)
        } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            Some(Self::TarGz)
        } else {
            None
        }
    }
}

/// Progress callback: `(done, total)` in format-specific units (entries for zip, compressed
/// bytes for tar.gz). Only the ratio is meaningful.
pub type ExtractProgress<'a> = &'a mut dyn FnMut(u64, u64);

/// Unpacks `archive` into the (existing, empty) folder `dest`. Blocking.
pub fn extract(
    archive: &Path,
    format: ArchiveFormat,
    dest: &Path,
    cancel: &AtomicBool,
    progress: ExtractProgress<'_>,
) -> Result<()> {
    match format {
        ArchiveFormat::Zip => extract_zip(archive, dest, cancel, progress),
        ArchiveFormat::TarGz => extract_tar_gz(archive, dest, cancel, progress),
    }
}

/// If the archive had exactly one top-level folder, that folder is the engine's real root.
pub fn content_root(dest: &Path) -> Result<PathBuf> {
    let mut entries = fs::read_dir(dest)?;
    let first = entries.next().transpose()?;
    let second = entries.next().transpose()?;
    match (first, second) {
        (Some(only), None) if only.file_type()?.is_dir() => Ok(only.path()),
        _ => Ok(dest.to_path_buf()),
    }
}

fn unsafe_entry(what: impl Into<String>) -> Error {
    Error::UnsafeArchive(what.into())
}

fn ensure_not_cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

/// Copies `reader` into `writer`, counting every byte against the global size cap.
fn copy_limited(
    reader: &mut impl Read,
    writer: &mut impl Write,
    written: &mut u64,
    cancel: &AtomicBool,
) -> Result<()> {
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        ensure_not_cancelled(cancel)?;
        let read = reader
            .read(&mut buffer)
            .map_err(|error| unsafe_entry(format!("unreadable data: {error}")))?;
        if read == 0 {
            return Ok(());
        }
        *written += u64::try_from(read).unwrap_or(u64::MAX);
        if *written > MAX_EXTRACTED_BYTES {
            return Err(unsafe_entry("the archive unpacks to an unreasonable size"));
        }
        writer.write_all(&buffer[..read])?;
    }
}

/// Rejects `path` if any folder between `root` and it is a symbolic link, so nothing is ever
/// written through a link an earlier entry planted.
fn ensure_plain_parents(root: &Path, path: &Path) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| unsafe_entry(path.display().to_string()))?;
    let Some(parent) = relative.parent() else {
        return Ok(());
    };
    let mut current = root.to_path_buf();
    for component in parent.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(unsafe_entry(format!("{} goes through a link", path.display())));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn prepare_target(root: &Path, target: &Path) -> Result<()> {
    ensure_plain_parents(root, target)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(unsafe_entry(format!("{} replaces a link", target.display())))
        }
        _ => Ok(()),
    }
}

/// Turns an archive path into a relative path made only of normal components.
fn sanitize_relative(path: &Path) -> Result<PathBuf> {
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return Err(unsafe_entry(path.display().to_string())),
        }
    }
    Ok(clean)
}

/// Lexically checks that a relative link target never climbs out of `root`. This is sound
/// because [`ensure_plain_parents`] guarantees no folder on the way is itself a link.
#[cfg_attr(not(unix), allow(dead_code))]
fn link_stays_inside(root: &Path, link_path: &Path, target: &Path) -> bool {
    let Ok(relative) = link_path.strip_prefix(root) else {
        return false;
    };
    let mut depth = relative.components().count().saturating_sub(1);
    for component in target.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

#[cfg(unix)]
fn create_symlink_checked(root: &Path, link_path: &Path, target: &str) -> Result<()> {
    let target_path = Path::new(target);
    if target.is_empty() || !link_stays_inside(root, link_path, target_path) {
        return Err(unsafe_entry(format!("link {} -> {target}", link_path.display())));
    }
    prepare_target(root, link_path)?;
    std::os::unix::fs::symlink(target_path, link_path)?;
    Ok(())
}

#[cfg(not(unix))]
fn create_symlink_checked(_root: &Path, link_path: &Path, target: &str) -> Result<()> {
    Err(unsafe_entry(format!(
        "link {} -> {target} (symbolic links are not supported on this platform)",
        link_path.display()
    )))
}

#[cfg(unix)]
fn apply_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    // Keep the permission bits only: never setuid/setgid/sticky.
    fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777))?;
    Ok(())
}

#[cfg(not(unix))]
fn apply_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

fn archive_error(error: impl std::fmt::Display) -> Error {
    unsafe_entry(format!("unreadable archive: {error}"))
}

// ---------------------------------------------------------------------------------- zip ----

fn is_symlink_mode(mode: u32) -> bool {
    mode & 0o170_000 == 0o120_000
}

fn extract_zip(
    archive: &Path,
    dest: &Path,
    cancel: &AtomicBool,
    progress: ExtractProgress<'_>,
) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(archive)?).map_err(archive_error)?;
    let total = u64::try_from(zip.len()).unwrap_or(u64::MAX);
    let mut written = 0_u64;

    for index in 0..zip.len() {
        ensure_not_cancelled(cancel)?;
        let mut entry = zip.by_index(index).map_err(archive_error)?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(unsafe_entry(entry.name().to_owned()));
        };
        let target = dest.join(relative);
        let mode = entry.unix_mode();

        if entry.is_dir() {
            ensure_plain_parents(dest, &target)?;
            fs::create_dir_all(&target)?;
        } else if mode.is_some_and(is_symlink_mode) {
            let mut link = String::new();
            entry
                .by_ref()
                .take(MAX_LINK_TARGET_BYTES)
                .read_to_string(&mut link)
                .map_err(archive_error)?;
            create_symlink_checked(dest, &target, &link)?;
        } else {
            prepare_target(dest, &target)?;
            let mut out = File::create(&target)?;
            copy_limited(&mut entry, &mut out, &mut written, cancel)?;
            drop(out);
            if let Some(mode) = mode {
                apply_mode(&target, mode)?;
            }
        }
        progress(u64::try_from(index + 1).unwrap_or(u64::MAX), total);
    }
    Ok(())
}

// -------------------------------------------------------------------------------- tar.gz ----

/// Counts the compressed bytes pulled from the file, for progress reporting.
struct CountingReader<R> {
    inner: R,
    count: Arc<AtomicU64>,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.count
            .fetch_add(u64::try_from(read).unwrap_or(0), Ordering::Relaxed);
        Ok(read)
    }
}

fn extract_tar_gz(
    archive: &Path,
    dest: &Path,
    cancel: &AtomicBool,
    progress: ExtractProgress<'_>,
) -> Result<()> {
    let file = File::open(archive)?;
    let total = file.metadata()?.len();
    let count = Arc::new(AtomicU64::new(0));
    let reader = CountingReader {
        inner: file,
        count: Arc::clone(&count),
    };
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(reader));
    let mut written = 0_u64;

    for entry in tar.entries().map_err(archive_error)? {
        ensure_not_cancelled(cancel)?;
        let mut entry = entry.map_err(archive_error)?;
        let entry_type = entry.header().entry_type();

        // Extended headers are bookkeeping for the next entry, not files.
        if entry_type.is_pax_global_extensions()
            || entry_type.is_pax_local_extensions()
            || entry_type.is_gnu_longname()
            || entry_type.is_gnu_longlink()
        {
            continue;
        }

        let path = entry.path().map_err(archive_error)?.into_owned();
        let relative = sanitize_relative(&path)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(&relative);

        if entry_type.is_dir() {
            ensure_plain_parents(dest, &target)?;
            fs::create_dir_all(&target)?;
        } else if entry_type.is_file() {
            prepare_target(dest, &target)?;
            let mode = entry.header().mode().ok();
            let mut out = File::create(&target)?;
            copy_limited(&mut entry, &mut out, &mut written, cancel)?;
            drop(out);
            if let Some(mode) = mode {
                apply_mode(&target, mode)?;
            }
        } else if entry_type.is_symlink() {
            let link = entry
                .link_name()
                .map_err(archive_error)?
                .ok_or_else(|| unsafe_entry(format!("link {} has no target", path.display())))?
                .to_string_lossy()
                .into_owned();
            create_symlink_checked(dest, &target, &link)?;
        } else {
            return Err(unsafe_entry(format!(
                "{} is a hard link or special file",
                path.display()
            )));
        }
        progress(count.load(Ordering::Relaxed), total);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn no_progress() -> impl FnMut(u64, u64) {
        |_, _| {}
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, data) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }

    fn write_tar_gz(path: &Path, build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) {
        let mut builder = tar::Builder::new(Vec::new());
        build(&mut builder);
        let tar_bytes = builder.into_inner().unwrap();
        let mut encoder =
            flate2::write::GzEncoder::new(File::create(path).unwrap(), flate2::Compression::fast());
        encoder.write_all(&tar_bytes).unwrap();
        encoder.finish().unwrap();
    }

    fn add_file(builder: &mut tar::Builder<Vec<u8>>, name: &str, data: &[u8]) {
        let mut header = tar::Header::new_gnu();
        header.set_size(u64::try_from(data.len()).unwrap());
        header.set_mode(0o755);
        header.set_cksum();
        builder.append_data(&mut header, name, Cursor::new(data)).unwrap();
    }

    #[test]
    fn recognises_archive_formats() {
        assert_eq!(ArchiveFormat::from_path("/a/Engine.ZIP"), Some(ArchiveFormat::Zip));
        assert_eq!(ArchiveFormat::from_path("/a/e.tar.gz"), Some(ArchiveFormat::TarGz));
        assert_eq!(ArchiveFormat::from_path("/a/e.tgz"), Some(ArchiveFormat::TarGz));
        assert_eq!(ArchiveFormat::from_path("/a/e.exe"), None);
    }

    #[test]
    fn extracts_a_zip() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.zip");
        write_zip(&archive, &[("Engine/bin/run", b"hello"), ("Engine/readme.txt", b"hi")]);
        let dest = dir.path().join("out");
        fs::create_dir(&dest).unwrap();

        extract(&archive, ArchiveFormat::Zip, &dest, &AtomicBool::new(false), &mut no_progress())
            .unwrap();

        assert_eq!(fs::read(dest.join("Engine/bin/run")).unwrap(), b"hello");
        assert_eq!(content_root(&dest).unwrap(), dest.join("Engine"));
    }

    #[test]
    fn zip_with_a_parent_directory_entry_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("evil.zip");
        write_zip(&archive, &[("../evil.txt", b"x")]);
        let dest = dir.path().join("out");
        fs::create_dir(&dest).unwrap();

        let result =
            extract(&archive, ArchiveFormat::Zip, &dest, &AtomicBool::new(false), &mut no_progress());

        assert!(matches!(result, Err(Error::UnsafeArchive(_))));
        assert!(!dir.path().join("evil.txt").exists());
    }

    #[test]
    fn extracts_a_tar_gz_with_a_safe_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.tar.gz");
        write_tar_gz(&archive, |builder| {
            add_file(builder, "Engine/lib/libcore.so.1", b"lib");
            let mut link = tar::Header::new_gnu();
            link.set_entry_type(tar::EntryType::Symlink);
            link.set_size(0);
            link.set_cksum();
            builder
                .append_link(&mut link, "Engine/lib/libcore.so", "libcore.so.1")
                .unwrap();
        });
        let dest = dir.path().join("out");
        fs::create_dir(&dest).unwrap();

        let result = extract(
            &archive,
            ArchiveFormat::TarGz,
            &dest,
            &AtomicBool::new(false),
            &mut no_progress(),
        );

        #[cfg(unix)]
        {
            result.unwrap();
            assert_eq!(fs::read(dest.join("Engine/lib/libcore.so")).unwrap(), b"lib");
        }
        #[cfg(not(unix))]
        assert!(result.is_err());
    }

    #[test]
    fn tar_symlink_escaping_the_folder_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("evil.tar.gz");
        write_tar_gz(&archive, |builder| {
            let mut link = tar::Header::new_gnu();
            link.set_entry_type(tar::EntryType::Symlink);
            link.set_size(0);
            link.set_cksum();
            builder.append_link(&mut link, "Engine/escape", "../../outside").unwrap();
        });
        let dest = dir.path().join("out");
        fs::create_dir(&dest).unwrap();

        let result = extract(
            &archive,
            ArchiveFormat::TarGz,
            &dest,
            &AtomicBool::new(false),
            &mut no_progress(),
        );

        assert!(matches!(result, Err(Error::UnsafeArchive(_))));
    }

    #[test]
    fn link_containment_is_checked_lexically() {
        let root = Path::new("/r");
        assert!(link_stays_inside(root, Path::new("/r/a/l"), Path::new("../b")));
        assert!(link_stays_inside(root, Path::new("/r/l"), Path::new("a/b")));
        assert!(!link_stays_inside(root, Path::new("/r/l"), Path::new("../x")));
        assert!(!link_stays_inside(root, Path::new("/r/a/l"), Path::new("../../x")));
        assert!(!link_stays_inside(root, Path::new("/r/l"), Path::new("/etc/passwd")));
    }

    #[test]
    fn cancelling_stops_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.zip");
        write_zip(&archive, &[("a.txt", b"x")]);
        let dest = dir.path().join("out");
        fs::create_dir(&dest).unwrap();

        let result =
            extract(&archive, ArchiveFormat::Zip, &dest, &AtomicBool::new(true), &mut no_progress());

        assert!(matches!(result, Err(Error::Cancelled)));
    }

    #[test]
    fn content_root_keeps_multi_entry_archives_as_they_are() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), b"").unwrap();
        fs::write(dir.path().join("b"), b"").unwrap();
        assert_eq!(content_root(dir.path()).unwrap(), dir.path());
    }
}
