//! State Persistence Layer
//!
//! Handles loading and saving application state with:
//! - Versioned serialization
//! - Backup on parse failure

use super::app_state::AppState;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// State version for schema tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StateVersion(pub u32);

impl StateVersion {
    pub const CURRENT: StateVersion = StateVersion(1);

    pub fn is_current(&self) -> bool {
        *self == Self::CURRENT
    }
}

impl std::fmt::Display for StateVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}", self.0)
    }
}

/// Persistence errors
#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

pub type PersistenceResult<T> = Result<T, PersistenceError>;

/// Load application state from file
///
/// Handles:
/// - File not found (returns default)
/// - Parse errors (backs up corrupt file)
/// - Version mismatch (returns default with warning)
pub fn load_state(base_dir: &Path, file_name: &str) -> PersistenceResult<AppState> {
    let path = state_file_path(base_dir, file_name);

    if !path.exists() {
        log::info!("State file not found, using defaults");
        return Ok(AppState::default());
    }

    let mut file = File::open(&path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    match serde_json::from_str::<AppState>(&contents) {
        Ok(state) => {
            log::info!("Loaded state from {:?} ({})", path, state.version);

            if state.version != StateVersion::CURRENT {
                log::warn!(
                    "State file has {} but current is {}. \
                     Returning default state.",
                    state.version,
                    StateVersion::CURRENT
                );
                return Ok(AppState::default());
            }

            Ok(state)
        }
        Err(e) => {
            drop(file);

            let backup_name = format!(
                "{}_backup_{}.json",
                file_name.trim_end_matches(".json"),
                chrono::Utc::now().timestamp()
            );
            let backup_path = state_file_path(base_dir, &backup_name);

            if let Err(rename_err) = std::fs::rename(&path, &backup_path) {
                log::warn!(
                    "Failed to backup corrupted state file '{}': {}",
                    path.display(),
                    rename_err
                );
            } else {
                log::info!(
                    "Backed up corrupted state file to '{}'",
                    backup_path.display()
                );
            }

            log::warn!("Parse error: {}, using default state", e);
            Ok(AppState::default())
        }
    }
}

/// Save application state to file
///
/// Creates parent directories if needed.
pub fn save_state(state: &AppState, base_dir: &Path, file_name: &str) -> PersistenceResult<()> {
    let path = state_file_path(base_dir, file_name);

    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(state)?;

    let tmp_path = path.with_extension("tmp");
    let mut file = File::create(&tmp_path)?;
    file.write_all(json.as_bytes())?;
    file.flush()?;

    std::fs::rename(&tmp_path, &path)?;

    log::info!("Saved state to {:?}", path);
    Ok(())
}

/// Path to state file under the given base directory.
fn state_file_path(base_dir: &Path, file_name: &str) -> PathBuf {
    base_dir.join(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_version() {
        let current = StateVersion::CURRENT;
        assert!(current.is_current());
        assert_eq!(current, StateVersion(1));
    }

    #[test]
    fn test_save_and_load() {
        let state = AppState::default();
        let temp_dir = std::env::temp_dir();
        let temp_file = format!("test-state-{}.json", chrono::Utc::now().timestamp());

        save_state(&state, &temp_dir, &temp_file).unwrap();

        let loaded = load_state(&temp_dir, &temp_file).unwrap();
        assert_eq!(loaded.version, state.version);

        let path = state_file_path(&temp_dir, &temp_file);
        let _ = std::fs::remove_file(path);
    }
}
