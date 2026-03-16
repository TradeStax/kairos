//! Built-in study implementations and the [`StudyRegistry`] factory.
//!
//! Studies are organized by category:
//!
//! - [`trend`] — SMA, EMA, VWAP
//! - [`momentum`] — RSI, MACD, Stochastic
//! - [`volume`] — Volume, Delta, CVD, OBV
//! - [`volatility`] — ATR, Bollinger Bands
//! - [`orderflow`] — Footprint, VBP, Big Trades, Imbalance
//!
//! Use [`StudyRegistry::new()`] to create a registry pre-loaded with all
//! built-in studies, then call [`StudyRegistry::create()`] to instantiate
//! a study by its string ID.

mod registry;

pub mod momentum;
pub mod orderflow;
pub mod statistical;
pub mod trend;
pub mod volatility;
pub mod volume;

pub use momentum::{MacdStudy, RsiStudy, StochasticStudy};
pub use orderflow::{BigTradesStudy, FootprintStudy, ImbalanceStudy, SpeedOfTapeStudy, VbpStudy};
pub use statistical::IvbStudy;
pub use trend::{EmaStudy, SmaStudy, VwapStudy};
pub use volatility::{AtrStudy, BollingerStudy};
pub use volume::{CvdStudy, DeltaStudy, ObvStudy, VolumeStudy};

use crate::core::{Study, StudyCapabilities, StudyCategory, StudyPlacement};
use std::collections::HashMap;

/// Metadata about a registered study, used for catalog display and
/// filtering in the study selection UI.
#[derive(Debug, Clone)]
pub struct StudyInfo {
    /// Unique string identifier (e.g. `"sma"`, `"big_trades"`).
    pub id: String,
    /// Human-readable display name (e.g. `"Simple Moving Average"`).
    pub name: String,
    /// Functional category for grouping in the catalog.
    pub category: StudyCategory,
    /// Where the study renders relative to the chart.
    pub placement: StudyPlacement,
    /// Short description shown in the study picker tooltip.
    pub description: String,
    /// Optional feature flags for this study.
    pub capabilities: StudyCapabilities,
    /// Schema version for parameter persistence.
    pub config_version: u16,
}

impl StudyInfo {
    /// Build `StudyInfo` from a live study instance, pulling all
    /// fields from its [`StudyMetadata`](crate::core::StudyMetadata).
    pub fn from_study(study: &dyn Study) -> Self {
        let meta = study.metadata();
        StudyInfo {
            id: study.id().to_string(),
            name: meta.name.clone(),
            category: meta.category,
            placement: meta.placement,
            description: meta.description.clone(),
            capabilities: meta.capabilities.clone(),
            config_version: meta.config_version,
        }
    }
}

/// Factory registry for creating [`Study`] instances by string ID.
///
/// Pre-loaded with all built-in studies on construction via
/// `register_built_ins`. Custom studies
/// can be added at runtime via [`register()`](Self::register).
pub struct StudyRegistry {
    /// Closure factories keyed by study ID.
    factories: HashMap<String, Box<dyn Fn() -> Box<dyn Study> + Send + Sync>>,
    /// Catalog metadata keyed by study ID.
    info: HashMap<String, StudyInfo>,
}

impl StudyRegistry {
    /// Create a new registry with all built-in studies registered.
    pub fn new() -> Self {
        let mut registry = Self {
            factories: HashMap::new(),
            info: HashMap::new(),
        };

        registry::register_built_ins(&mut registry);

        registry
    }

    /// Register a study factory with explicit metadata.
    ///
    /// Prefer [`register_study`](Self::register_study) for built-in studies
    /// where metadata is derived automatically. Use this method for custom or
    /// external studies that need to provide their own [`StudyInfo`].
    pub fn register<F>(&mut self, id: &str, info: StudyInfo, factory: F)
    where
        F: Fn() -> Box<dyn Study> + Send + Sync + 'static,
    {
        self.factories.insert(id.to_string(), Box::new(factory));
        self.info.insert(id.to_string(), info);
    }

    /// Register a study factory, deriving [`StudyInfo`] from the study's
    /// own [`StudyMetadata`](crate::core::StudyMetadata).
    ///
    /// Creates a temporary instance to extract metadata, then stores the
    /// factory closure for future instantiation. This eliminates the need
    /// to duplicate study metadata in the registry.
    pub fn register_study<F>(&mut self, factory: F)
    where
        F: Fn() -> Box<dyn Study> + Send + Sync + 'static,
    {
        let instance = factory();
        let info = StudyInfo::from_study(instance.as_ref());
        let id = info.id.clone();
        self.factories.insert(id.clone(), Box::new(factory));
        self.info.insert(id, info);
    }

    /// Check if a study with the given ID is already registered.
    pub fn contains(&self, id: &str) -> bool {
        self.factories.contains_key(id)
    }

    /// Create a study instance by id.
    pub fn create(&self, id: &str) -> Option<Box<dyn Study>> {
        self.factories.get(id).map(|f| f())
    }

    /// List all registered studies.
    pub fn list(&self) -> Vec<StudyInfo> {
        let mut studies: Vec<_> = self.info.values().cloned().collect();
        studies.sort_by(|a, b| a.name.cmp(&b.name));
        studies
    }

    /// List studies filtered by category.
    pub fn list_by_category(&self, category: StudyCategory) -> Vec<StudyInfo> {
        let mut studies: Vec<_> = self
            .info
            .values()
            .filter(|info| info.category == category)
            .cloned()
            .collect();
        studies.sort_by(|a, b| a.name.cmp(&b.name));
        studies
    }

    /// List studies filtered by where they render on the chart.
    ///
    /// Returns entries matching the given [`StudyPlacement`], sorted
    /// alphabetically by name. Commonly used to populate the study
    /// picker UI which groups studies into Overlay, Panel, Background,
    /// and CandleReplace sections.
    pub fn list_by_placement(&self, placement: StudyPlacement) -> Vec<StudyInfo> {
        let mut studies: Vec<_> = self
            .info
            .values()
            .filter(|info| info.placement == placement)
            .cloned()
            .collect();
        studies.sort_by(|a, b| a.name.cmp(&b.name));
        studies
    }
}

impl Default for StudyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
