use std::{
    fs::File,
    io::Read,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use sha2::{Digest, Sha256};

use crate::{Error, Result};

const CHUNK: usize = 1024 * 1024;

/// Streams a file through SHA-256 and returns the lowercase hex digest. Blocking: call it from
/// `spawn_blocking`. `on_progress` receives the number of bytes hashed so far.
pub fn sha256_file(
    path: &Path,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u64),
) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; CHUNK];
    let mut done = 0_u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        done += u64::try_from(read).unwrap_or(u64::MAX);
        on_progress(done);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn hashes_a_file() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"abc").unwrap();
        let digest = sha256_file(file.path(), &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn stops_when_cancelled() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let result = sha256_file(file.path(), &AtomicBool::new(true), |_| {});
        assert!(matches!(result, Err(Error::Cancelled)));
    }
}
