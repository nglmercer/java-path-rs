//! Streaming, resumable-safe artifact download with checksum verification.

use crate::error::{Error, Result};
use crate::provision::checksum::verify_sha256;
use crate::provision::provider::JdkRelease;
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Progress reported while an artifact is downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Bytes written so far.
    pub downloaded: u64,
    /// Total bytes, when the server reports a length.
    pub total: Option<u64>,
}

/// Download `release` into `dir`, verifying its checksum before committing.
///
/// The bytes are streamed into a `.partial` file; only after verification is
/// it renamed to its final name, so a failed download never leaves a file
/// that could be mistaken for a good artifact.
pub async fn download_release(
    client: &reqwest::Client,
    release: &JdkRelease,
    dir: &Path,
    mut on_progress: impl FnMut(Progress),
) -> Result<PathBuf> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| Error::io(dir, e))?;

    let final_path = dir.join(&release.file_name);
    let partial_path = dir.join(format!("{}.partial", release.file_name));

    if final_path.is_file() {
        if let Some(expected) = &release.sha256 {
            if verify_sha256(&final_path, expected).is_ok() {
                return Ok(final_path);
            }
        }
    }

    let response = client
        .get(&release.url)
        .send()
        .await
        .map_err(|e| Error::Network(e.to_string()))?;
    if !response.status().is_success() {
        return Err(Error::Network(format!(
            "{} returned HTTP {}",
            release.url,
            response.status()
        )));
    }

    let total = response.content_length().or(release.size);
    let mut file = tokio::fs::File::create(&partial_path)
        .await
        .map_err(|e| Error::io(&partial_path, e))?;
    let mut downloaded = 0u64;
    let mut stream = response.bytes_stream();

    let result: Result<()> = async {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| Error::Network(e.to_string()))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| Error::io(&partial_path, e))?;
            downloaded += chunk.len() as u64;
            on_progress(Progress { downloaded, total });
        }
        file.flush()
            .await
            .map_err(|e| Error::io(&partial_path, e))?;
        Ok(())
    }
    .await;

    drop(file);
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&partial_path).await;
        return Err(e);
    }

    if let Some(expected) = &release.sha256 {
        verify_sha256(&partial_path, expected)?;
    }

    tokio::fs::rename(&partial_path, &final_path)
        .await
        .map_err(|e| Error::io(&final_path, e))?;
    Ok(final_path)
}
