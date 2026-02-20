//! State Migration Implementations
//!
//! This module contains all state migrations for upgrading between schema versions.
//! Each migration transforms the AppState from one version to the next.

use crate::state::app::AppState;
use crate::state::persistence::{PersistenceResult, StateMigration};

/// Register all migrations with the registry
///
/// This function should be called by the MigrationRegistry constructor
/// to register all available migrations.
pub fn register_all_migrations() -> Vec<Box<dyn StateMigration>> {
    vec![Box::new(MigrationV1ToV2)]
}

/// Migration v1 -> v2: Add data feed manager
///
/// Creates a DataFeedManager from existing configuration. If a Databento
/// API key is present, a default Databento feed is auto-created.
struct MigrationV1ToV2;

impl StateMigration for MigrationV1ToV2 {
    fn source_version(&self) -> u32 {
        1
    }

    fn to_version(&self) -> u32 {
        2
    }

    fn migrate(&self, mut state: AppState) -> PersistenceResult<AppState> {
        // Check if Databento API key is available
        let secrets = crate::secrets::SecretsManager::new();
        let has_databento_key = secrets.has_api_key(crate::secrets::ApiProvider::Databento);

        state.data_feeds = crate::feed::DataFeedManager::migrate_from_legacy(has_databento_key);

        log::info!(
            "Migration v1->v2: Created DataFeedManager with {} feed(s)",
            state.data_feeds.total_count()
        );

        Ok(state)
    }

    fn description(&self) -> &str {
        "Add data feed manager with migration from legacy config"
    }
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
