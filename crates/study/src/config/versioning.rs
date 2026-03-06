//! Parameter schema versioning and migration.
//!
//! [`ParameterSchema`] pairs parameter definitions with a version number
//! and a list of migration functions that upgrade configs from older
//! versions to the current one.

use super::store::StudyConfig;

/// A function that migrates a [`StudyConfig`] from one version to the next.
pub type ConfigMigration = fn(&mut StudyConfig);

/// Versioned parameter schema with migration support.
///
/// Studies that evolve their parameter set across releases can declare
/// a `ParameterSchema` with migrations. When a persisted config is
/// loaded with an older version, the schema applies migrations
/// sequentially to bring it up to date.
pub struct ParameterSchema {
    /// Current schema version.
    pub version: u16,
    /// Migration functions keyed by the version they upgrade FROM.
    pub migrations: Vec<(u16, ConfigMigration)>,
}

impl ParameterSchema {
    /// Apply all migrations from `from_version` up to the current version.
    pub fn migrate(&self, config: &mut StudyConfig, from_version: u16) {
        let mut sorted: Vec<_> = self
            .migrations
            .iter()
            .filter(|(v, _)| *v >= from_version)
            .collect();
        sorted.sort_by_key(|(v, _)| *v);

        for (_, migration) in sorted {
            migration(config);
        }
    }
}
