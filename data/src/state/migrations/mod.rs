//! State Migration Implementations
//!
//! This module contains all state migrations for upgrading between schema versions.
//! Each migration transforms the AppState from one version to the next.

use crate::state::app::AppState;
use crate::state::persistence::{PersistenceResult, StateMigration, StateVersion};

/// Register all migrations with the registry
///
/// This function should be called by the MigrationRegistry constructor
/// to register all available migrations.
pub fn register_all_migrations() -> Vec<Box<dyn StateMigration>> {
    vec![Box::new(MigrationV1ToV2), Box::new(MigrationV2ToV3)]
}

/// Migration v1 -> v2: Add data feed manager
///
/// Creates a DataFeedManager from existing configuration. If a Databento
/// API key is present, a default Databento feed is auto-created.
struct MigrationV1ToV2;

impl StateMigration for MigrationV1ToV2 {
    fn source_version(&self) -> StateVersion {
        StateVersion(1)
    }

    fn to_version(&self) -> StateVersion {
        StateVersion(2)
    }

    fn migrate(&self, mut state: AppState) -> PersistenceResult<AppState> {
        // Check if Databento API key is available (env only; keyring lives in GUI crate)
        let has_databento_key = std::env::var(crate::config::secrets::ApiProvider::Databento.env_var()).is_ok();

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

/// Migration v2 -> v3: Theme and colors now data-only (no iced_core)
///
/// Wire format is unchanged; version bump only.
struct MigrationV2ToV3;

impl StateMigration for MigrationV2ToV3 {
    fn source_version(&self) -> StateVersion {
        StateVersion(2)
    }

    fn to_version(&self) -> StateVersion {
        StateVersion(3)
    }

    fn migrate(&self, mut state: AppState) -> PersistenceResult<AppState> {
        state.version = StateVersion(3);
        log::info!("Migration v2->v3: Bumped state version (theme/colors now data-only)");
        Ok(state)
    }

    fn description(&self) -> &str {
        "Bump version for data-only theme and Rgba (no iced_core)"
    }
}

/// Base trait extensions for migrations
pub trait MigrationExt: StateMigration {
    /// Log the migration start
    fn log_start(&self) {
        log::info!(
            "Starting migration: {} → {}: {}",
            self.source_version(),
            self.to_version(),
            self.description()
        );
    }

    /// Log the migration completion
    fn log_complete(&self) {
        log::info!(
            "Completed migration: {} → {}",
            self.source_version(),
            self.to_version()
        );
    }

    /// Validate state before migration
    fn validate_pre(&self, state: &AppState) -> PersistenceResult<()> {
        if state.version != self.source_version() {
            return Err(crate::state::persistence::PersistenceError::Migration(
                format!(
                    "State version mismatch: expected {}, got {}",
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
