//! One delta package: a ZIP holding `delta.json` and the blobs it refers to.
//!
//! `delta.json` lists, per changed path, what to do: `patch` (a bsdiff patch against the file
//! of the older version), `replace` (the whole new file), `add` (a new file) or `remove`.
//! Unchanged files are not listed. The format is documented in Coreverse DB
//! (`docs/src/database/domains/releases.md`).

use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use zip::ZipArchive;

use super::{bad, manifest::safe_relative};
use crate::{Result, engine::metadata::is_sha256_hex, platform::Platform};

const MAX_DOC_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeltaOp {
    Patch,
    Replace,
    Add,
    Remove,
}

#[derive(Debug, Deserialize)]
pub struct DeltaEntry {
    pub path: String,
    pub op: DeltaOp,
    #[serde(default)]
    pub old_sha256: Option<String>,
    #[serde(default)]
    pub new_sha256: Option<String>,
    #[serde(default)]
    pub blob: Option<String>,
    /// `path` as a checked relative path; filled in by [`open`].
    #[serde(skip)]
    pub rel: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct DeltaDoc {
    pub format: u32,
    pub algorithm: String,
    pub from_version: String,
    pub to_version: String,
    pub os: String,
    pub architecture: String,
    pub entries: Vec<DeltaEntry>,
}

fn zip_error(error: &zip::result::ZipError) -> crate::Error {
    bad(format!("the delta package is damaged: {error}"))
}

fn valid_blob_name(name: &str) -> bool {
    name.strip_prefix("blobs/")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

/// Opens a delta package and checks its `delta.json` against what the plan promised
/// (`from` to `to` on `platform`). Nothing is applied yet.
pub fn open(
    path: &Path,
    from: &str,
    to: &str,
    platform: Platform,
) -> Result<(ZipArchive<File>, DeltaDoc)> {
    let mut archive = ZipArchive::new(File::open(path)?).map_err(|error| zip_error(&error))?;

    let mut bytes = Vec::new();
    {
        let mut entry = archive
            .by_name("delta.json")
            .map_err(|error| zip_error(&error))?;
        (&mut entry)
            .take(MAX_DOC_BYTES + 1)
            .read_to_end(&mut bytes)?;
    }
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_DOC_BYTES {
        return Err(bad("delta.json is too large".to_owned()));
    }
    let mut doc: DeltaDoc = serde_json::from_slice(&bytes)
        .map_err(|error| bad(format!("delta.json is not valid: {error}")))?;

    if doc.format != 1 || doc.algorithm != "bsdiff" {
        return Err(bad(format!(
            "unsupported delta package (format {}, algorithm {})",
            doc.format, doc.algorithm
        )));
    }
    if doc.from_version != from
        || doc.to_version != to
        || doc.os != platform.os
        || doc.architecture != platform.architecture
    {
        return Err(bad("the delta package is for a different update".to_owned()));
    }

    // In this version of the zip crate each name is a `Result` (a name can fail to decode).
    let mut names: HashSet<String> = HashSet::new();
    for name in archive.file_names() {
        names.insert(name.map_err(|error| zip_error(&error))?.into_owned());
    }
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for entry in &mut doc.entries {
        entry.rel = safe_relative(&entry.path)?;
        if !seen.insert(entry.rel.clone()) {
            return Err(bad(format!("{} appears twice in the package", entry.path)));
        }
        let hash_ok = |value: &Option<String>| value.as_deref().is_some_and(is_sha256_hex);
        let needs_old = entry.op == DeltaOp::Patch;
        let needs_new = entry.op != DeltaOp::Remove;
        if (needs_old && !hash_ok(&entry.old_sha256)) || (needs_new && !hash_ok(&entry.new_sha256))
        {
            return Err(bad(format!("{} has a missing or invalid hash", entry.path)));
        }
        if needs_new {
            let blob_ok = entry
                .blob
                .as_deref()
                .is_some_and(|name| valid_blob_name(name) && names.contains(name));
            if !blob_ok {
                return Err(bad(format!("{} refers to a missing blob", entry.path)));
            }
        }
    }
    Ok((archive, doc))
}

/// Reads one blob, refusing anything larger than `limit` bytes.
pub fn read_blob(archive: &mut ZipArchive<File>, name: &str, limit: u64) -> Result<Vec<u8>> {
    let mut entry = archive.by_name(name).map_err(|error| zip_error(&error))?;
    if entry.size() > limit {
        return Err(bad(format!("{name} is too large to apply in memory")));
    }
    let mut bytes = Vec::new();
    (&mut entry).take(limit + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(bad(format!("{name} is too large to apply in memory")));
    }
    Ok(bytes)
}

#[cfg(test)]
pub(super) mod test_support {
    use std::{fs::File, io::Write, path::Path};

    /// Writes a delta package: `delta.json` plus the given `(name, bytes)` blobs.
    pub fn write_package(path: &Path, doc_json: &str, blobs: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("delta.json", options).unwrap();
        writer.write_all(doc_json.as_bytes()).unwrap();
        for (name, bytes) in blobs {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support::write_package, *};

    fn linux() -> Platform {
        Platform::from_rust_consts("linux", "x86_64").unwrap()
    }

    fn doc(entries: &str) -> String {
        format!(
            r#"{{"format":1,"algorithm":"bsdiff","from_version":"1.0.0","to_version":"1.1.0","os":"linux","architecture":"x86_64","entries":[{entries}]}}"#
        )
    }

    fn sha(c: char) -> String {
        c.to_string().repeat(64)
    }

    #[test]
    fn opens_a_valid_package_and_reads_blobs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("delta.zip");
        let entries = format!(
            r#"{{"path":"bin/engine","op":"patch","old_sha256":"{}","new_sha256":"{}","blob":"blobs/0"}},{{"path":"old.txt","op":"remove"}}"#,
            sha('a'),
            sha('b')
        );
        write_package(&path, &doc(&entries), &[("blobs/0", b"patch-bytes")]);

        let (mut archive, parsed) = open(&path, "1.0.0", "1.1.0", linux()).unwrap();

        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].op, DeltaOp::Patch);
        assert_eq!(
            read_blob(&mut archive, "blobs/0", 1024).unwrap(),
            b"patch-bytes"
        );
        assert!(read_blob(&mut archive, "blobs/0", 3).is_err());
    }

    #[test]
    fn rejects_a_package_for_another_update() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("delta.zip");
        write_package(&path, &doc(""), &[]);
        assert!(open(&path, "1.0.0", "1.2.0", linux()).is_err());
        assert!(open(&path, "0.9.0", "1.1.0", linux()).is_err());
    }

    #[test]
    fn rejects_unsafe_paths_missing_blobs_and_bad_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("delta.zip");

        let traversal = format!(
            r#"{{"path":"../x","op":"add","new_sha256":"{}","blob":"blobs/0"}}"#,
            sha('a')
        );
        write_package(&path, &doc(&traversal), &[("blobs/0", b"x")]);
        assert!(open(&path, "1.0.0", "1.1.0", linux()).is_err());

        let missing_blob = format!(
            r#"{{"path":"x","op":"add","new_sha256":"{}","blob":"blobs/9"}}"#,
            sha('a')
        );
        write_package(&path, &doc(&missing_blob), &[("blobs/0", b"x")]);
        assert!(open(&path, "1.0.0", "1.1.0", linux()).is_err());

        let bad_hash = r#"{"path":"x","op":"add","new_sha256":"nope","blob":"blobs/0"}"#;
        write_package(&path, &doc(bad_hash), &[("blobs/0", b"x")]);
        assert!(open(&path, "1.0.0", "1.1.0", linux()).is_err());

        let odd_blob = format!(
            r#"{{"path":"x","op":"add","new_sha256":"{}","blob":"../delta.json"}}"#,
            sha('a')
        );
        write_package(&path, &doc(&odd_blob), &[("blobs/0", b"x")]);
        assert!(open(&path, "1.0.0", "1.1.0", linux()).is_err());
    }

    #[test]
    fn rejects_a_file_that_is_not_a_zip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("delta.zip");
        std::fs::write(&path, b"not a zip").unwrap();
        assert!(open(&path, "1.0.0", "1.1.0", linux()).is_err());
    }
}
