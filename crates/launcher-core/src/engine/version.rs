use std::path::{Path, PathBuf};

use crate::{Error, Result, platform::Platform};

/// Accepts only plain `MAJOR.MINOR.PATCH` versions and returns them normalised. The version ends
/// up in file system paths, so nothing else (pre-release tags, `..`, separators) may pass.
pub fn normalize_version(raw: &str) -> Result<String> {
    let invalid = || Error::InvalidVersion(raw.to_owned());
    let parsed = semver::Version::parse(raw.trim()).map_err(|_| invalid())?;
    if !parsed.pre.is_empty() || !parsed.build.is_empty() {
        return Err(invalid());
    }
    Ok(parsed.to_string())
}

/// Orders two normalised versions; unparsable ones sort last.
pub fn compare_desc(a: &str, b: &str) -> std::cmp::Ordering {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(a), Ok(b)) => b.cmp(&a),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => std::cmp::Ordering::Equal,
    }
}

/// Where engine files live below the launcher's app data folder.
#[derive(Debug, Clone)]
pub struct EnginePaths {
    root: PathBuf,
}

impl EnginePaths {
    #[must_use]
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.to_path_buf(),
        }
    }

    /// Final folder of an installed version. `version` must be normalised.
    #[must_use]
    pub fn install_dir(&self, version: &str) -> PathBuf {
        self.root.join("engines").join(version)
    }

    /// Folder for half-unpacked installs; each install gets its own temporary subfolder.
    #[must_use]
    pub fn staging_root(&self) -> PathBuf {
        self.root.join("engines").join(".staging")
    }

    /// Partial or complete download of one incremental-update file (a manifest or a delta
    /// package). The name is its SHA-256, so equal names mean equal bytes. `sha256` must be
    /// validated.
    #[must_use]
    pub fn delta_file(&self, sha256: &str) -> PathBuf {
        self.root
            .join("downloads")
            .join(format!("delta-{sha256}.part"))
    }

    /// Partial or complete download of one artifact. The checksum is part of the name, so a
    /// re-published build never resumes on top of the old bytes. `sha256` must be validated.
    #[must_use]
    pub fn part_file(&self, version: &str, platform: Platform, sha256: &str) -> PathBuf {
        let short = sha256.get(..16).unwrap_or(sha256);
        self.root.join("downloads").join(format!(
            "engine-{version}-{}-{}-{short}.part",
            platform.os, platform.architecture
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_versions() {
        assert_eq!(normalize_version("1.2.3").unwrap(), "1.2.3");
        assert_eq!(normalize_version(" 0.10.0 ").unwrap(), "0.10.0");
    }

    #[test]
    fn rejects_anything_that_could_leave_the_engines_folder() {
        for bad in [
            "1.2",
            "1.2.3-beta",
            "1.2.3+build",
            "../1.2.3",
            "1.2.3/x",
            "",
            "01.2.3",
        ] {
            assert!(normalize_version(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn sorts_newest_first() {
        let mut versions = vec!["1.2.0", "1.10.0", "0.9.9"];
        versions.sort_by(|a, b| compare_desc(a, b));
        assert_eq!(versions, ["1.10.0", "1.2.0", "0.9.9"]);
    }

    #[test]
    fn part_file_name_includes_the_checksum_prefix() {
        let paths = EnginePaths::new(Path::new("/data"));
        let platform = Platform::from_rust_consts("linux", "x86_64").unwrap();
        let part = paths.part_file("1.0.0", platform, &"ab".repeat(32));
        assert!(part.ends_with("engine-1.0.0-linux-x86_64-abababababababab.part"));
    }
}
