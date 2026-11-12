//! The file manifest of one engine build: every file of the installed tree with its SHA-256.
//!
//! A manifest becomes a directory tree on the user's disk, so it is validated as strictly as
//! an archive: relative `/`-separated paths without `..`, no duplicates, no entry below a
//! symbolic link, and symbolic links that cannot point outside the engine folder. The format is
//! documented in Coreverse DB (`docs/src/database/domains/releases.md`).

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use super::bad;
use crate::{Result, engine::metadata::is_sha256_hex, platform::Platform};

/// A manifest larger than this is not read (it would only list a few hundred thousand files).
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    #[default]
    File,
    Symlink,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    #[serde(default)]
    pub kind: FileKind,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub mode: Option<u32>,
    #[serde(default)]
    pub target: Option<String>,
    /// `path` as a checked relative path; filled in by [`Manifest::parse`].
    #[serde(skip)]
    pub rel: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub version: String,
    pub os: String,
    pub architecture: String,
    pub files: Vec<ManifestFile>,
}

/// Turns a `/`-separated manifest or delta path into a relative [`PathBuf`], refusing anything
/// that could leave the engine folder.
pub fn safe_relative(raw: &str) -> Result<PathBuf> {
    if raw.is_empty() || raw.contains('\\') || raw.contains('\0') {
        return Err(bad(format!("unsafe path {raw:?}")));
    }
    let mut clean = PathBuf::new();
    for segment in raw.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || (cfg!(windows) && segment.contains(':'))
        {
            return Err(bad(format!("unsafe path {raw:?}")));
        }
        clean.push(segment);
    }
    Ok(clean)
}

/// A symbolic link may point to a relative target that stays inside the engine folder.
fn symlink_target_is_safe(link: &Path, target: &str) -> bool {
    if target.is_empty()
        || target.contains('\\')
        || target.contains('\0')
        || target.starts_with('/')
    {
        return false;
    }
    let mut depth = i64::try_from(link.components().count().saturating_sub(1)).unwrap_or(i64::MAX);
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => depth += 1,
        }
    }
    true
}

impl Manifest {
    /// Parses and validates a manifest. It must describe exactly `version` for `platform`.
    pub fn parse(bytes: &[u8], version: &str, platform: Platform) -> Result<Self> {
        let mut manifest: Self = serde_json::from_slice(bytes)
            .map_err(|error| bad(format!("the manifest is not valid: {error}")))?;
        if manifest.format != 1 {
            return Err(bad(format!(
                "unsupported manifest format {}",
                manifest.format
            )));
        }
        if manifest.version != version
            || manifest.os != platform.os
            || manifest.architecture != platform.architecture
        {
            return Err(bad("the manifest describes a different build".to_owned()));
        }
        if manifest.files.is_empty() {
            return Err(bad("the manifest lists no files".to_owned()));
        }

        let mut seen: HashSet<PathBuf> = HashSet::new();
        for file in &mut manifest.files {
            file.rel = safe_relative(&file.path)?;
            if !seen.insert(file.rel.clone()) {
                return Err(bad(format!("{} is listed twice", file.path)));
            }
            match file.kind {
                FileKind::File => {
                    if !is_sha256_hex(&file.sha256) {
                        return Err(bad(format!("{} has no valid sha256", file.path)));
                    }
                }
                FileKind::Symlink => {
                    let target = file.target.as_deref().unwrap_or_default();
                    if !symlink_target_is_safe(&file.rel, target) {
                        return Err(bad(format!(
                            "{} is a symbolic link with an unsafe target",
                            file.path
                        )));
                    }
                }
            }
        }
        // A file or link can never also be a folder that holds other entries.
        for file in &manifest.files {
            for ancestor in file.rel.ancestors().skip(1) {
                if seen.contains(ancestor) {
                    return Err(bad(format!(
                        "{} lies below another manifest entry",
                        file.path
                    )));
                }
            }
        }
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linux() -> Platform {
        Platform::from_rust_consts("linux", "x86_64").unwrap()
    }

    fn manifest_json(files: &str) -> String {
        format!(
            r#"{{"format":1,"version":"1.2.0","os":"linux","architecture":"x86_64","files":[{files}]}}"#
        )
    }

    fn file(path: &str) -> String {
        format!(
            r#"{{"path":"{path}","kind":"file","size":3,"sha256":"{}","mode":493}}"#,
            "a".repeat(64)
        )
    }

    #[test]
    fn safe_paths_are_relative_and_normalised() {
        assert_eq!(
            safe_relative("bin/engine").unwrap(),
            Path::new("bin").join("engine")
        );
        for bad_path in [
            "",
            "/etc/passwd",
            "../x",
            "a/../b",
            "a//b",
            "a/./b",
            "a\\b",
            "a/",
            "./a",
        ] {
            assert!(
                safe_relative(bad_path).is_err(),
                "{bad_path:?} must be rejected"
            );
        }
    }

    #[test]
    fn parses_a_valid_manifest() {
        let json = manifest_json(&format!(
            "{},{}",
            file("bin/engine"),
            r#"{"path":"lib/a.so","kind":"symlink","target":"a.so.1"}"#
        ));
        let manifest = Manifest::parse(json.as_bytes(), "1.2.0", linux()).unwrap();
        assert_eq!(manifest.files.len(), 2);
        assert_eq!(manifest.files[0].rel, Path::new("bin").join("engine"));
        assert_eq!(manifest.files[1].kind, FileKind::Symlink);
    }

    #[test]
    fn rejects_a_manifest_for_another_build() {
        let json = manifest_json(&file("a"));
        assert!(Manifest::parse(json.as_bytes(), "1.3.0", linux()).is_err());
        let mac = Platform::from_rust_consts("macos", "aarch64").unwrap();
        assert!(Manifest::parse(json.as_bytes(), "1.2.0", mac).is_err());
    }

    #[test]
    fn rejects_unsafe_duplicate_and_nested_entries() {
        let unsafe_path = manifest_json(&file("../evil"));
        assert!(Manifest::parse(unsafe_path.as_bytes(), "1.2.0", linux()).is_err());

        let duplicate = manifest_json(&format!("{},{}", file("a"), file("a")));
        assert!(Manifest::parse(duplicate.as_bytes(), "1.2.0", linux()).is_err());

        let nested = manifest_json(&format!("{},{}", file("a"), file("a/b")));
        assert!(Manifest::parse(nested.as_bytes(), "1.2.0", linux()).is_err());

        let below_link = manifest_json(&format!(
            "{},{}",
            r#"{"path":"d","kind":"symlink","target":"e"}"#,
            file("d/x")
        ));
        assert!(Manifest::parse(below_link.as_bytes(), "1.2.0", linux()).is_err());
    }

    #[test]
    fn rejects_bad_hashes_and_escaping_links() {
        let bad_hash = manifest_json(r#"{"path":"a","kind":"file","size":1,"sha256":"XYZ"}"#);
        assert!(Manifest::parse(bad_hash.as_bytes(), "1.2.0", linux()).is_err());

        // From `lib/l`, `../x` is still inside the engine folder (`<root>/x`); two levels up is not.
        for target in ["/etc/passwd", "../../outside", "a/../../../outside"] {
            let json = manifest_json(&format!(
                r#"{{"path":"lib/l","kind":"symlink","target":"{target}"}}"#
            ));
            assert!(
                Manifest::parse(json.as_bytes(), "1.2.0", linux()).is_err(),
                "{target} must be rejected"
            );
        }
        let inside = manifest_json(r#"{"path":"lib/l","kind":"symlink","target":"../bin/x"}"#);
        assert!(Manifest::parse(inside.as_bytes(), "1.2.0", linux()).is_ok());
    }
}
