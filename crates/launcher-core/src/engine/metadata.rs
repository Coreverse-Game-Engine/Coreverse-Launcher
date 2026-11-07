use std::time::Duration;

use reqwest::{StatusCode, Url};
use serde::Deserialize;

use super::extract::ArchiveFormat;
use crate::{Error, Result, platform::Platform};

const METADATA_TIMEOUT: Duration = Duration::from_secs(30);

/// The part of Coreverse DB's `Release` the installer needs. Unknown fields are ignored.
#[derive(Debug, Deserialize)]
pub struct ReleaseMetadata {
    pub version: String,
    pub status: String,
    pub artifacts: Vec<ArtifactMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ArtifactMetadata {
    pub os: String,
    pub architecture: String,
    pub download_url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

/// An artifact whose fields have been checked and parsed.
#[derive(Debug)]
pub struct ValidatedArtifact {
    pub url: Url,
    pub format: ArchiveFormat,
}

/// Reads one release from `GET {api_base_url}/releases/{version}`. The endpoint is public, so no
/// token is sent.
pub async fn fetch_release(
    client: &reqwest::Client,
    api_base_url: &str,
    version: &str,
) -> Result<ReleaseMetadata> {
    let response = client
        .get(format!("{api_base_url}/releases/{version}"))
        .timeout(METADATA_TIMEOUT)
        .send()
        .await?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Err(Error::ReleaseNotFound(version.to_owned()));
    }
    if !status.is_success() {
        return Err(Error::Http {
            status: status.as_u16(),
        });
    }
    let release: ReleaseMetadata = response.json().await?;
    if release.version != version {
        return Err(Error::InvalidMetadata(format!(
            "asked for {version}, the server returned {}",
            release.version
        )));
    }
    Ok(release)
}

/// Picks the build for this platform. The DB has at most one per `(os, architecture)`.
pub fn select_artifact(release: &ReleaseMetadata, platform: Platform) -> Result<&ArtifactMetadata> {
    release
        .artifacts
        .iter()
        .find(|artifact| artifact.os == platform.os && artifact.architecture == platform.architecture)
        .ok_or_else(|| Error::NoArtifact {
            version: release.version.clone(),
            os: platform.os.to_owned(),
            architecture: platform.architecture.to_owned(),
        })
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Checks everything the installer relies on before a single byte is downloaded.
///
/// The SHA-256 is the real security anchor; the URL rules only keep obviously wrong sources
/// (plain HTTP, embedded credentials, unknown archive types) out. Debug builds additionally
/// allow `http://` so a local test server works.
pub fn validate_artifact(artifact: &ArtifactMetadata) -> Result<ValidatedArtifact> {
    if !is_sha256_hex(&artifact.sha256) {
        return Err(Error::InvalidMetadata("sha256 is not 64 lowercase hex digits".to_owned()));
    }
    if artifact.size_bytes == 0 {
        return Err(Error::InvalidMetadata("size_bytes must be greater than zero".to_owned()));
    }

    let url = Url::parse(&artifact.download_url)
        .map_err(|_| Error::InvalidUrl(artifact.download_url.clone()))?;
    let scheme_ok = url.scheme() == "https" || (cfg!(debug_assertions) && url.scheme() == "http");
    if !scheme_ok
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(Error::InvalidUrl(artifact.download_url.clone()));
    }

    let format = ArchiveFormat::from_path(url.path())
        .ok_or_else(|| Error::UnsupportedArchive(url.path().to_owned()))?;
    Ok(ValidatedArtifact { url, format })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(url: &str) -> ArtifactMetadata {
        ArtifactMetadata {
            os: "linux".to_owned(),
            architecture: "x86_64".to_owned(),
            download_url: url.to_owned(),
            sha256: "a".repeat(64),
            size_bytes: 10,
        }
    }

    #[test]
    fn accepts_a_github_release_url() {
        let validated =
            validate_artifact(&artifact("https://github.com/o/r/releases/download/v1/engine.zip"))
                .unwrap();
        assert_eq!(validated.format, ArchiveFormat::Zip);
    }

    #[test]
    fn rejects_credentials_unknown_archives_and_bad_hashes() {
        assert!(validate_artifact(&artifact("https://u:p@host/engine.zip")).is_err());
        assert!(validate_artifact(&artifact("https://host/engine.exe")).is_err());
        assert!(validate_artifact(&artifact("ftp://host/engine.zip")).is_err());
        let mut bad_hash = artifact("https://host/engine.zip");
        bad_hash.sha256 = "ABC".to_owned();
        assert!(validate_artifact(&bad_hash).is_err());
        let mut empty = artifact("https://host/engine.zip");
        empty.size_bytes = 0;
        assert!(validate_artifact(&empty).is_err());
    }

    #[test]
    fn selects_the_artifact_for_the_platform() {
        let release = ReleaseMetadata {
            version: "1.0.0".to_owned(),
            status: "stable".to_owned(),
            artifacts: vec![artifact("https://host/a.zip")],
        };
        let linux = Platform::from_rust_consts("linux", "x86_64").unwrap();
        let mac = Platform::from_rust_consts("macos", "aarch64").unwrap();
        assert!(select_artifact(&release, linux).is_ok());
        assert!(matches!(
            select_artifact(&release, mac),
            Err(Error::NoArtifact { .. })
        ));
    }

    #[test]
    fn parses_the_api_shape_and_ignores_extra_fields() {
        let json = r#"{"version":"1.0.0","status":"stable","release_date":"2026-01-01",
            "release_notes_summary":null,"artifacts":[{"os":"linux","architecture":"x86_64",
            "download_url":"https://h/e.zip","sha256":"aa","size_bytes":5,
            "min_requirements":{},"compiler":{}}]}"#;
        let release: ReleaseMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(release.artifacts[0].size_bytes, 5);
    }
}
