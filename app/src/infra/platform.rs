//! Platform I/O utilities
//!
//! Data directory paths and OS-level operations that don't belong in the
//! pure-domain `kairos-data` crate.

use std::path::PathBuf;

/// Get data directory path
pub fn data_path(path_name: Option<&str>) -> PathBuf {
    let base = if let Ok(path) = std::env::var("KAIROS_DATA_PATH") {
        PathBuf::from(path)
    } else {
        let data_dir = dirs_next::data_dir().unwrap_or_else(|| PathBuf::from("."));
        data_dir.join("kairos")
    };

    if let Some(path_name) = path_name {
        base.join(path_name)
    } else {
        base
    }
}

/// Open data folder in system file browser
pub fn open_data_folder() -> Result<(), data::DataError> {
    let pathbuf = data_path(None);

    if pathbuf.exists() {
        open::that(&pathbuf).map_err(|e| {
            data::DataError::State(format!("Failed to open folder: {}", e))
        })?;
        log::info!("Opened data folder: {:?}", pathbuf);
        Ok(())
    } else {
        Err(data::DataError::State(format!(
            "Data folder does not exist: {:?}",
            pathbuf
        )))
    }
}
