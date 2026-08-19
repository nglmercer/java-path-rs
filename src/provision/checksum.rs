//! SHA-256 verification of downloaded artifacts.

use crate::error::{Error, Result};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Compute the SHA-256 digest of a file as a lowercase hex string.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| Error::io(path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf).map_err(|e| Error::io(path, e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

/// Verify a file against an expected digest, deleting it on mismatch.
pub fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    let actual = sha256_file(path)?;
    if actual.eq_ignore_ascii_case(expected.trim()) {
        return Ok(());
    }
    // A corrupt or tampered artifact must never be left behind for reuse.
    let _ = std::fs::remove_file(path);
    Err(Error::ChecksumMismatch {
        expected: expected.trim().to_ascii_lowercase(),
        actual,
    })
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}
