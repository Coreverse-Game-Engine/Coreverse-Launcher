//! Asking Coreverse DB how to get from an installed version to a newer one.
//!
//! `GET {api}/releases/{to}/delta?from=&os=&architecture=` answers either with a chain of delta
//! packages plus the target's file manifest, or with advice to download the full build. Every
//! file in the answer carries its SHA-256, which is what the rest of the update is checked
//! against; the URLs only have to be plausible.

use std::time::Duration;

use reqwest::{StatusCode, Url};
use serde::Deserialize;

use super::{bad, manifest::MAX_MANIFEST_BYTES};
use crate::{
    Error, Result,
    engine::metadata::{is_sha256_hex, validate_download_url},
    platform::Platform,
};

const PLAN_TIMEOUT: Duration = Duration::from_secs(30);

/// The part of Coreverse DB's `DeltaPlan` the launcher needs. Unknown fields are ignored.
#[derive(Debug, Deserialize)]
pub struct DeltaPlan {
    pub recommended: String,
    pub reason: String,
    pub to_manifest: Option<RemoteFile>,
    #[serde(default)]
    pub steps: Vec<RemoteStep>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteFile {
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteStep {
    pub from_version: String,
    pub to_version: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub algorithm: String,
    pub format_version: u32,
}

/// A file to download, with its address parsed and its hash and size checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub url: Url,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepDownload {
    pub download: Download,
    pub from: String,
    pub to: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidatedPlan {
    pub manifest: Download,
    pub steps: Vec<StepDownload>,
}

/// Reads the plan. `Ok(None)` means the server has no plan for this pair (an older server, or
/// an unknown release), which simply means "download the full build".
pub async fn fetch(
    client: &reqwest::Client,
    api_base_url: &str,
    from: &str,
    to: &str,
    platform: Platform,
) -> Result<Option<DeltaPlan>> {
    // Versions are normalised and platform names are fixed strings: nothing needs escaping.
    let url = format!(
        "{api_base_url}/releases/{to}/delta?from={from}&os={}&architecture={}",
        platform.os, platform.architecture
    );
    let response = client.get(url).timeout(PLAN_TIMEOUT).send().await?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(Error::Http {
            status: status.as_u16(),
        });
    }
    Ok(Some(response.json().await?))
}

fn download(url: &str, sha256: &str, size_bytes: u64, what: &str) -> Result<Download> {
    if !is_sha256_hex(sha256) || size_bytes == 0 {
        return Err(bad(format!(
            "the plan has an invalid hash or size for {what}"
        )));
    }
    Ok(Download {
        url: validate_download_url(url)?,
        sha256: sha256.to_owned(),
        size_bytes,
    })
}

impl DeltaPlan {
    /// Checks that the plan really leads from `base` to `target` in one unbroken chain.
    /// Call it only for plans that recommend a delta.
    pub fn validate(&self, base: &str, target: &str) -> Result<ValidatedPlan> {
        if self.recommended != "delta" {
            return Err(bad(format!(
                "the plan recommends a full download ({})",
                self.reason
            )));
        }
        let (Some(first), Some(last)) = (self.steps.first(), self.steps.last()) else {
            return Err(bad("the plan has no steps".to_owned()));
        };
        if first.from_version != base || last.to_version != target {
            return Err(bad(
                "the plan does not lead from the installed version to the target".to_owned(),
            ));
        }
        for pair in self.steps.windows(2) {
            if pair[0].to_version != pair[1].from_version {
                return Err(bad("the plan has a gap between two steps".to_owned()));
            }
        }

        let manifest = self
            .to_manifest
            .as_ref()
            .ok_or_else(|| bad("the plan has no manifest".to_owned()))?;
        if manifest.size_bytes > MAX_MANIFEST_BYTES {
            return Err(bad("the manifest is too large".to_owned()));
        }
        let manifest = download(
            &manifest.url,
            &manifest.sha256,
            manifest.size_bytes,
            "the manifest",
        )?;

        let mut steps = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            if step.algorithm != "bsdiff" || step.format_version != 1 {
                return Err(bad(format!(
                    "unsupported delta package ({} v{})",
                    step.algorithm, step.format_version
                )));
            }
            steps.push(StepDownload {
                download: download(&step.url, &step.sha256, step.size_bytes, "a delta package")?,
                from: step.from_version.clone(),
                to: step.to_version.clone(),
            });
        }
        Ok(ValidatedPlan { manifest, steps })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(from: &str, to: &str) -> String {
        format!(
            r#"{{"from_version":"{from}","to_version":"{to}","url":"https://h/{from}-{to}.zip","sha256":"{}","size_bytes":10,"algorithm":"bsdiff","format_version":1}}"#,
            "b".repeat(64)
        )
    }

    fn plan(steps: &[String]) -> DeltaPlan {
        let json = format!(
            r#"{{"recommended":"delta","reason":"delta","from_version":"1.0.0","to_version":"1.2.0",
            "full":{{"download_url":"https://h/e.zip","sha256":"{a}","size_bytes":100}},
            "to_manifest":{{"url":"https://h/m.json","sha256":"{c}","size_bytes":20}},
            "from_manifest":null,"delta_bytes":20,"max_steps":5,"max_ratio":0.6,
            "steps":[{}]}}"#,
            steps.join(","),
            a = "a".repeat(64),
            c = "c".repeat(64)
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn accepts_a_contiguous_chain() {
        let plan = plan(&[step("1.0.0", "1.1.0"), step("1.1.0", "1.2.0")]);
        let validated = plan.validate("1.0.0", "1.2.0").unwrap();
        assert_eq!(validated.steps.len(), 2);
        assert_eq!(validated.steps[1].to, "1.2.0");
        assert_eq!(validated.manifest.size_bytes, 20);
    }

    #[test]
    fn rejects_chains_that_do_not_fit() {
        let gap = plan(&[step("1.0.0", "1.1.0"), step("1.1.5", "1.2.0")]);
        assert!(gap.validate("1.0.0", "1.2.0").is_err());

        let wrong_start = plan(&[step("0.9.0", "1.2.0")]);
        assert!(wrong_start.validate("1.0.0", "1.2.0").is_err());

        let wrong_end = plan(&[step("1.0.0", "1.1.0")]);
        assert!(wrong_end.validate("1.0.0", "1.2.0").is_err());

        let empty = plan(&[]);
        assert!(empty.validate("1.0.0", "1.2.0").is_err());
    }

    #[test]
    fn rejects_a_full_recommendation_and_unsupported_packages() {
        let mut full = plan(&[step("1.0.0", "1.2.0")]);
        full.recommended = "full".to_owned();
        assert!(full.validate("1.0.0", "1.2.0").is_err());

        let mut other = plan(&[step("1.0.0", "1.2.0")]);
        other.steps[0].algorithm = "xdelta".to_owned();
        assert!(other.validate("1.0.0", "1.2.0").is_err());

        let mut bad_hash = plan(&[step("1.0.0", "1.2.0")]);
        bad_hash.steps[0].sha256 = "nope".to_owned();
        assert!(bad_hash.validate("1.0.0", "1.2.0").is_err());

        let mut bad_url = plan(&[step("1.0.0", "1.2.0")]);
        bad_url.steps[0].url = "ftp://h/x.zip".to_owned();
        assert!(bad_url.validate("1.0.0", "1.2.0").is_err());

        let mut no_manifest = plan(&[step("1.0.0", "1.2.0")]);
        no_manifest.to_manifest = None;
        assert!(no_manifest.validate("1.0.0", "1.2.0").is_err());
    }

    #[test]
    fn parses_the_api_shape_and_ignores_extra_fields() {
        let plan = plan(&[step("1.0.0", "1.2.0")]);
        assert_eq!(plan.recommended, "delta");
        assert_eq!(plan.steps.len(), 1);
    }
}
