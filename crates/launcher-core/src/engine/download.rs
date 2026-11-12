use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use futures_util::StreamExt;
use reqwest::{StatusCode, Url, header::RANGE};
use tokio::{fs, io::AsyncWriteExt};

use super::progress::{InstallPhase, Reporter};
use crate::{Error, Result};

/// Where one file sits inside a bigger progress bar: reports are `base + received` out of
/// `total`. A plain download uses `base: 0` and the file size as `total`.
#[derive(Debug, Clone, Copy)]
pub struct ProgressSpan {
    pub base: u64,
    pub total: u64,
}

/// A connection that delivers nothing for this long is treated as dead.
const STALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Downloads `url` into `part_path`, resuming a partial file from an earlier attempt when the
/// server supports range requests. The file is *not* verified here; the caller hashes it.
///
/// A download that ends early keeps its partial file so the next attempt can continue.
pub async fn download(
    client: &reqwest::Client,
    url: &Url,
    part_path: &Path,
    expected_size: u64,
    cancel: &AtomicBool,
    reporter: &Reporter,
    span: ProgressSpan,
) -> Result<()> {
    if let Some(parent) = part_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let mut offset = match fs::metadata(part_path).await {
        Ok(metadata) => metadata.len(),
        Err(_) => 0,
    };
    if offset > expected_size {
        fs::remove_file(part_path).await?;
        offset = 0;
    }
    if offset == expected_size {
        // A previous attempt already fetched everything; only verification is left.
        reporter.phase(
            InstallPhase::Downloading,
            span.base + expected_size,
            span.total,
        );
        return Ok(());
    }

    let response = loop {
        let mut request = client.get(url.clone());
        if offset > 0 {
            request = request.header(RANGE, format!("bytes={offset}-"));
        }
        let response = request.send().await?;
        if response.status() == StatusCode::RANGE_NOT_SATISFIABLE && offset > 0 {
            // The partial file does not fit this server's file: start over.
            fs::remove_file(part_path).await?;
            offset = 0;
            continue;
        }
        break response;
    };

    let status = response.status();
    if !status.is_success() {
        return Err(Error::Http {
            status: status.as_u16(),
        });
    }

    let mut file = if status == StatusCode::PARTIAL_CONTENT && offset > 0 {
        fs::OpenOptions::new().append(true).open(part_path).await?
    } else {
        // 200 means the server ignored the range (or there was nothing to resume).
        offset = 0;
        fs::File::create(part_path).await?
    };

    let mut received = offset;
    reporter.phase(InstallPhase::Downloading, span.base + received, span.total);

    let mut stream = response.bytes_stream();
    loop {
        let next = tokio::time::timeout(STALL_TIMEOUT, stream.next())
            .await
            .map_err(|_| Error::Network("the connection stalled".to_owned()))?;
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk?;
        if cancel.load(Ordering::Relaxed) {
            file.flush().await?;
            return Err(Error::Cancelled);
        }
        received += u64::try_from(chunk.len()).unwrap_or(u64::MAX);
        if received > expected_size {
            drop(file);
            fs::remove_file(part_path).await?;
            return Err(Error::SizeMismatch {
                expected: expected_size,
                actual: received,
            });
        }
        file.write_all(&chunk).await?;
        reporter.tick(InstallPhase::Downloading, span.base + received, span.total);
    }
    file.flush().await?;

    if received < expected_size {
        return Err(Error::Network(
            "the download was interrupted; try again to resume it".to_owned(),
        ));
    }
    reporter.phase(InstallPhase::Downloading, span.base + received, span.total);
    Ok(())
}
