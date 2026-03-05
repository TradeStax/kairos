use serde::{Deserialize, Serialize};

/// Where the app checks for updates.
/// The CI pipeline uploads this to the Generic Package Registry at the "latest" path.
pub const UPDATE_MANIFEST_URL: &str = "https://gitlab.com/api/v4/projects/77621610/packages/generic/kairos/latest/update-manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub release_date: String,
    pub minimum_version: Option<String>,
    pub release_notes: String,
    pub platforms: std::collections::HashMap<String, PlatformRelease>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformRelease {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub enum UpdateEvent {
    CheckComplete(Result<UpdateInfo, String>),
    DownloadProgress { downloaded: u64, total: u64 },
    DownloadComplete(Result<std::path::PathBuf, String>),
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: semver::Version,
    pub new_version: semver::Version,
    pub release_date: String,
    pub release_notes: String,
    pub download_url: String,
    pub sha256: String,
    pub size: u64,
    pub is_critical: bool,
}

/// Returns the target triple for the current platform.
pub fn current_platform_target() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "aarch64-pc-windows-msvc"
    }
    #[cfg(target_os = "macos")]
    {
        "universal-apple-darwin"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "aarch64-unknown-linux-gnu"
    }
}

/// Check for updates by fetching the manifest.
pub async fn check_for_update(current_version_str: &str) -> Result<Option<UpdateInfo>, String> {
    let current = semver::Version::parse(current_version_str)
        .map_err(|e| format!("Invalid current version: {e}"))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let manifest: UpdateManifest = client
        .get(UPDATE_MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch update manifest: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Invalid manifest JSON: {e}"))?;

    let new_version = semver::Version::parse(&manifest.version)
        .map_err(|e| format!("Invalid manifest version: {e}"))?;

    if new_version <= current {
        return Ok(None);
    }

    let target = current_platform_target();
    let platform = manifest
        .platforms
        .get(target)
        .ok_or_else(|| format!("No release available for platform: {target}"))?;

    let is_critical = manifest
        .minimum_version
        .as_ref()
        .is_some_and(|min| semver::Version::parse(min).is_ok_and(|min_v| current < min_v));

    Ok(Some(UpdateInfo {
        current_version: current,
        new_version,
        release_date: manifest.release_date,
        release_notes: manifest.release_notes,
        download_url: platform.url.clone(),
        sha256: platform.sha256.clone(),
        size: platform.size,
        is_critical,
    }))
}

/// Download update archive with streaming progress.
/// Sends progress events via the provided sender.
pub async fn download_update(
    url: &str,
    dest: &std::path::Path,
    expected_sha256: &str,
    tx: &tokio::sync::mpsc::UnboundedSender<UpdateEvent>,
) -> Result<std::path::PathBuf, String> {
    use futures::StreamExt;
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncWriteExt;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {e}"))?;

    let total = response.content_length().unwrap_or(0);

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create update directory: {e}"))?;
    }

    let tmp_path = dest.with_extension("tmp");
    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| format!("Failed to create temp file: {e}"))?;

    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download stream error: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Write error: {e}"))?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;

        let _ = tx.send(UpdateEvent::DownloadProgress { downloaded, total });
    }

    file.flush()
        .await
        .map_err(|e| format!("Flush error: {e}"))?;
    drop(file);

    // Verify checksum
    let hash = format!("{:x}", hasher.finalize());
    if hash != expected_sha256 {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(format!(
            "Checksum mismatch: expected {expected_sha256}, got {hash}"
        ));
    }

    tokio::fs::rename(&tmp_path, dest)
        .await
        .map_err(|e| format!("Failed to finalize download: {e}"))?;

    Ok(dest.to_path_buf())
}

/// Extract a .zip or .tar.gz archive to the staging directory.
pub fn extract_archive(
    archive_path: &std::path::Path,
    staging_dir: &std::path::Path,
) -> Result<(), String> {
    let lossy = archive_path.to_string_lossy();

    if lossy.ends_with(".tar.gz") || lossy.ends_with(".tgz") {
        let file = std::fs::File::open(archive_path)
            .map_err(|e| format!("Failed to open archive: {e}"))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive
            .unpack(staging_dir)
            .map_err(|e| format!("Failed to extract tar.gz: {e}"))?;
    } else if lossy.ends_with(".zip") {
        let file = std::fs::File::open(archive_path)
            .map_err(|e| format!("Failed to open archive: {e}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {e}"))?;
        archive
            .extract(staging_dir)
            .map_err(|e| format!("Failed to extract zip: {e}"))?;
    } else {
        let ext = archive_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("(none)");
        return Err(format!("Unsupported archive format: {ext}"));
    }

    Ok(())
}
