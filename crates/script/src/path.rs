//! Data directory path resolution for script cache and user scripts.
//!
//! Uses the same convention as the main app: KAIROS_DATA_PATH env, or
//! platform data dir under "kairos".

use std::path::PathBuf;

/// Returns the base data directory, then joins path_name if given.
///
/// Respects KAIROS_DATA_PATH or falls back to platform data dir via dirs_next.
pub fn data_path(path_name: Option<&str>) -> PathBuf {
    let base = if let Ok(path) = std::env::var("KAIROS_DATA_PATH") {
        PathBuf::from(path)
    } else {
        let data_dir = dirs_next::data_dir().unwrap_or_else(|| PathBuf::from("."));
        data_dir.join("kairos")
    };

    if let Some(name) = path_name {
        base.join(name)
    } else {
        base
    }
}

