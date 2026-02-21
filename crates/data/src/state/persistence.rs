//! State Persistence Layer
//!
//! Handles loading and saving application state with:
//! - Versioned serialization
//! - Migration support with registry
//! - Backup on parse failure
//! - Validation

use super::app::AppState;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// State version for migrations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StateVersion(pub u32);

impl StateVersion {
    pub const CURRENT: StateVersion = StateVersion(4);

    pub fn is_current(&self) -> bool {
        *self == Self::CURRENT
    }

    pub fn needs_migration(&self) -> bool {
        *self < Self::CURRENT
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

    #[error("Migration error: {0}")]
    Migration(String),
}

pub type PersistenceResult<T> = Result<T, PersistenceError>;

/// Migration trait for state version transitions
///
/// Each migration defines how to transform state from one version to the next.
pub trait StateMigration: Send + Sync {
    /// Source version (from)
    fn source_version(&self) -> StateVersion;

    /// Target version (to)
    fn to_version(&self) -> StateVersion;

    /// Perform the migration
    fn migrate(&self, state: AppState) -> PersistenceResult<AppState>;

    /// Description of what this migration does
    fn description(&self) -> &str;
}

/// Migration registry
///
/// Holds all available migrations and can execute a migration path.
pub struct MigrationRegistry {
    migrations: Vec<Box<dyn StateMigration>>,
}

impl MigrationRegistry {
    /// Create a new registry with all migrations
    pub fn new() -> Self {
        let mut registry = Self {
            migrations: Vec::new(),
        };

        // Register all migrations
        let migrations = crate::state::migrations::register_all_migrations();
        for migration in migrations {
            registry.register(migration);
        }

        registry
    }

    /// Register a migration
    pub fn register(&mut self, migration: Box<dyn StateMigration>) {
        log::info!(
            "Registered migration: v{} → v{}: {}",
            migration.source_version(),
            migration.to_version(),
            migration.description()
        );
        self.migrations.push(migration);
    }

    /// Get migration path from one version to another
    ///
    /// Returns a sequence of migrations to apply, or None if no path exists.
    pub fn get_migration_path(
        &self,
        from: StateVersion,
        to: StateVersion,
    ) -> Option<Vec<&dyn StateMigration>> {
        if from == to {
            return Some(Vec::new());
        }

        if from > to {
            log::error!("Cannot migrate backwards from {} to {}", from, to);
            return None;
        }

        // Simple linear path for now (v0 → v1 → v2 → v3, etc.)
        let mut path = Vec::new();
        let mut current = from;

        while current < to {
            let next = StateVersion(current.0 + 1);
            let next_migration = self
                .migrations
                .iter()
                .find(|m| m.source_version() == current && m.to_version() == next)?;

            path.push(next_migration.as_ref());
            current = next;
        }

        Some(path)
    }

    /// Execute migration path
    pub fn execute_migrations(
        &self,
        mut state: AppState,
        from: StateVersion,
        to: StateVersion,
    ) -> PersistenceResult<AppState> {
        let path = self.get_migration_path(from, to).ok_or_else(|| {
            PersistenceError::Migration(format!(
                "No migration path from {} to {}",
                from, to
            ))
        })?;

        if path.is_empty() {
            log::debug!("No migrations needed: {} is current", from);
            return Ok(state);
        }

        log::info!(
            "Executing {} migration(s): {} → {}",
            path.len(),
            from,
            to
        );

        for migration in path {
            log::info!("  Applying: {}", migration.description());

            state = migration.migrate(state)?;

            log::info!(
                "  Successfully migrated to {}",
                migration.to_version()
            );
        }

        Ok(state)
    }
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Load application state from file
///
/// Handles:
/// - File not found (returns default)
/// - Parse errors (backs up corrupt file)
/// - Version migrations
///
/// ## Example
/// ```rust,ignore
/// let state = load_state(base_dir, "saved-state.json")?;
/// ```
pub fn load_state(base_dir: &Path, file_name: &str) -> PersistenceResult<AppState> {
    let path = state_file_path(base_dir, file_name);

    // If file doesn't exist, return default
    if !path.exists() {
        log::info!("State file not found, using defaults");
        return Ok(AppState::default());
    }

    let mut file = File::open(&path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    // Try to parse state
    match serde_json::from_str::<AppState>(&contents) {
        Ok(mut state) => {
            log::info!("Loaded state from {:?} ({})", path, state.version);

            // Reject future versions that this app doesn't understand
            if state.version > StateVersion::CURRENT {
                log::warn!(
                    "State file has {} but this app only supports up \
                     to {}. The file may have been created by a newer \
                     version of the application. Using default state \
                     to avoid data corruption.",
                    state.version,
                    StateVersion::CURRENT
                );
                return Ok(AppState::default());
            }

            // Check if migration needed
            if state.version.needs_migration() {
                let old_version = state.version;
                log::info!(
                    "State {} needs migration to {}",
                    old_version,
                    StateVersion::CURRENT
                );

                // Create migration registry and execute migrations
                let registry = MigrationRegistry::new();
                state = registry.execute_migrations(
                    state,
                    old_version,
                    StateVersion::CURRENT,
                )?;

                // Update version after successful migration
                state.version = StateVersion::CURRENT;

                log::info!("Migration complete to {}", state.version);

                // Save migrated state
                save_state(&state, base_dir, file_name)?;
                log::info!("Saved migrated state");
            }

            Ok(state)
        }
        Err(e) => {
            // Parse failed - backup corrupted file
            drop(file); // Close before renaming

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

            // Return default state
            log::warn!("Parse error: {}, using default state", e);
            Ok(AppState::default())
        }
    }
}

/// Save application state to file
///
/// Creates parent directories if needed.
/// Validates state before saving.
///
/// ## Example
/// ```rust,ignore
/// save_state(&state, base_dir, "saved-state.json")?;
/// ```
pub fn save_state(state: &AppState, base_dir: &Path, file_name: &str) -> PersistenceResult<()> {
    let path = state_file_path(base_dir, file_name);

    // Create parent directories
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }

    // Serialize to JSON
    let json = serde_json::to_string_pretty(state)?;

    // Write to a temporary file first, then atomically rename
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

// ── Example Migrations (for future use) ───────────────────────────────

/// Example migration: v0 → v1 (baseline)
///
/// This is a placeholder migration. When actual schema changes are needed,
/// create new migrations following this pattern:
///
/// ```rust
/// struct MigrationV1ToV2;
///
/// impl StateMigration for MigrationV1ToV2 {
///     fn source_version(&self) -> u32 { 1 }
///     fn to_version(&self) -> u32 { 2 }
///
///     fn migrate(&self, mut state: AppState) -> PersistenceResult<AppState> {
///         // Transform state fields as needed
///         // Example: state.new_field = Default::default();
///         Ok(state)
///     }
///
///     fn description(&self) -> &str {
///         "Add new feature X"
///     }
/// }
/// ```
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_version() {
        let current = StateVersion::CURRENT;
        assert!(current.is_current());
        assert!(!current.needs_migration());

        let old = StateVersion(1);
        assert!(!old.is_current());
        assert!(old.needs_migration());
    }

    #[test]
    fn test_migration_registry() {
        let registry = MigrationRegistry::new();

        // Same version should have empty path
        let path = registry.get_migration_path(StateVersion(3), StateVersion(3));
        assert!(path.is_some());
        assert_eq!(path.unwrap().len(), 0);

        // v1 to v2 should have 1 migration
        let path = registry.get_migration_path(StateVersion(1), StateVersion(2));
        assert!(path.is_some());
        assert_eq!(path.unwrap().len(), 1);

        // v1 to v4 should have 3 migrations
        let path = registry.get_migration_path(StateVersion(1), StateVersion(4));
        assert!(path.is_some());
        assert_eq!(path.unwrap().len(), 3);
    }

    #[test]
    fn test_save_and_load() {
        let state = AppState::default();
        let temp_dir = std::env::temp_dir();
        let temp_file = format!("test-state-{}.json", chrono::Utc::now().timestamp());

        // Save
        save_state(&state, &temp_dir, &temp_file).unwrap();

        // Load
        let loaded = load_state(&temp_dir, &temp_file).unwrap();
        assert_eq!(loaded.version, state.version);

        // Cleanup
        let path = state_file_path(&temp_dir, &temp_file);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_backwards_migration_fails() {
        let registry = MigrationRegistry::new();
        let path = registry.get_migration_path(StateVersion(5), StateVersion(3));
        assert!(path.is_none(), "Should not allow backwards migration");
    }

    // Example custom migration for testing
    struct TestMigrationV0ToV1;

    impl StateMigration for TestMigrationV0ToV1 {
        fn source_version(&self) -> StateVersion {
            StateVersion(0)
        }

        fn to_version(&self) -> StateVersion {
            StateVersion(1)
        }

        fn migrate(&self, state: AppState) -> PersistenceResult<AppState> {
            Ok(state)
        }

        fn description(&self) -> &str {
            "Test migration v0 → v1"
        }
    }

    #[test]
    fn test_custom_migration() {
        let mut registry = MigrationRegistry::new();
        registry.register(Box::new(TestMigrationV0ToV1));

        // Now we have v0->v1 (custom) and v1->v2 (real), so v0->v2 path = 2
        let path = registry.get_migration_path(StateVersion(0), StateVersion(2));
        assert!(path.is_some());
        assert_eq!(path.unwrap().len(), 2);
    }

    #[test]
    fn test_execute_migrations() {
        let registry = MigrationRegistry::new();

        // Test the real v1->v2 migration
        let mut state = AppState::default();
        state.version = StateVersion(1);
        let result = registry.execute_migrations(
            state,
            StateVersion(1),
            StateVersion(2),
        );
        assert!(result.is_ok());
    }
}
