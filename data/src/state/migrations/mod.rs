//! State Migration Implementations
//!
//! This module contains all state migrations for upgrading between schema versions.
//! Each migration transforms the AppState from one version to the next.

use crate::state::app_state::AppState;
use crate::state::persistence::{PersistenceResult, StateMigration};

/// Register all migrations with the registry
///
/// This function should be called by the MigrationRegistry constructor
/// to register all available migrations.
pub fn register_all_migrations() -> Vec<Box<dyn StateMigration>> {
    vec![
        // Add new migrations here as they are created:
        // Box::new(v1_to_v2::MigrationV1ToV2),
        // etc.
    ]
}

/// Base trait extensions for migrations
pub trait MigrationExt: StateMigration {
    /// Log the migration start
    fn log_start(&self) {
        log::info!(
            "Starting migration: v{} → v{}: {}",
            self.source_version(),
            self.to_version(),
            self.description()
        );
    }

    /// Log the migration completion
    fn log_complete(&self) {
        log::info!(
            "Completed migration: v{} → v{}",
            self.source_version(),
            self.to_version()
        );
    }

    /// Validate state before migration
    fn validate_pre(&self, state: &AppState) -> PersistenceResult<()> {
        if state.version != self.source_version() {
            return Err(crate::state::persistence::PersistenceError::Migration(
                format!(
                    "State version mismatch: expected v{}, got v{}",
                    self.source_version(),
                    state.version
                ),
            ));
        }
        Ok(())
    }

    /// Validate state after migration
    fn validate_post(&self, _state: &AppState) -> PersistenceResult<()> {
        // Add any post-migration validation here
        Ok(())
    }
}

// Implement MigrationExt for all types that implement StateMigration
impl<T: StateMigration> MigrationExt for T {}
