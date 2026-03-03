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
    fn test_state_version_display() {
        assert_eq!(StateVersion(1).to_string(), "v1");
        assert_eq!(StateVersion(42).to_string(), "v42");
    }

    #[test]
    fn test_state_version_ordering() {
        assert!(StateVersion(1) < StateVersion(2));
        assert!(StateVersion(2) > StateVersion(1));
        assert_eq!(StateVersion(1), StateVersion(1));
    }

    #[test]
    fn test_state_version_not_current() {
        assert!(!StateVersion(0).is_current());
        assert!(!StateVersion(99).is_current());
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

    #[test]
    fn test_load_missing_file_returns_default() {
        let temp_dir = std::env::temp_dir();
        let loaded = load_state(&temp_dir, "nonexistent_file_12345.json").unwrap();
        assert_eq!(loaded.version, StateVersion::CURRENT);
    }

    #[test]
    fn test_load_corrupt_json_returns_default() {
        let temp_dir = std::env::temp_dir();
        let temp_file = format!("test-corrupt-{}.json", chrono::Utc::now().timestamp());
        let path = state_file_path(&temp_dir, &temp_file);

        // Write invalid JSON
        std::fs::write(&path, "{ not valid json }}}").unwrap();

        let loaded = load_state(&temp_dir, &temp_file).unwrap();
        assert_eq!(loaded.version, StateVersion::CURRENT);

        // Clean up (backup file created by load_state)
        let _ = std::fs::remove_file(&path);
        // Also try to clean the backup
        for entry in std::fs::read_dir(&temp_dir).into_iter().flatten() {
            if let Ok(entry) = entry {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("test-corrupt-") && name.contains("_backup_") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    #[test]
    fn test_save_creates_parent_dirs() {
        let temp_dir = std::env::temp_dir();
        let sub_dir = temp_dir.join(format!(
            "kairos_test_nested_{}",
            chrono::Utc::now().timestamp()
        ));
        let temp_file = "state.json";

        let state = AppState::default();
        save_state(&state, &sub_dir, temp_file).unwrap();

        let loaded = load_state(&sub_dir, temp_file).unwrap();
        assert_eq!(loaded.version, state.version);

        // Clean up
        let _ = std::fs::remove_dir_all(&sub_dir);
    }

    #[test]
    fn test_roundtrip_preserves_fields() {
        let mut state = AppState::default();
        state.trade_fetch_enabled = true;
        state.databento_config.cache_max_days = 180;
        state.ai_preferences.temperature = 0.7;

        let temp_dir = std::env::temp_dir();
        let temp_file = format!("test-fields-{}.json", chrono::Utc::now().timestamp());

        save_state(&state, &temp_dir, &temp_file).unwrap();
        let loaded = load_state(&temp_dir, &temp_file).unwrap();

        assert!(loaded.trade_fetch_enabled);
        assert_eq!(loaded.databento_config.cache_max_days, 180);
        assert!((loaded.ai_preferences.temperature - 0.7).abs() < 0.01);

        let path = state_file_path(&temp_dir, &temp_file);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_persistence_error_display() {
        let err = PersistenceError::InvalidPath("bad/path".to_string());
        assert!(err.to_string().contains("bad/path"));
    }
}
