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
pub mod trend;
pub mod volatility;
pub mod volume;

pub use momentum::{MacdStudy, RsiStudy, StochasticStudy};
pub use orderflow::{BigTradesStudy, FootprintStudy, ImbalanceStudy, SpeedOfTapeStudy, VbpStudy};
pub use trend::{EmaStudy, SmaStudy, VwapStudy};
pub use volatility::{AtrStudy, BollingerStudy};
pub use volume::{CvdStudy, DeltaStudy, ObvStudy, VolumeStudy};

use crate::core::{Study, StudyCategory, StudyPlacement};
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

    /// Register a study factory.
    pub fn register<F>(&mut self, id: &str, info: StudyInfo, factory: F)
    where
        F: Fn() -> Box<dyn Study> + Send + Sync + 'static,
    {
        self.factories.insert(id.to_string(), Box::new(factory));
        self.info.insert(id.to_string(), info);
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
mod tests {
    use super::*;
    use crate::core::{StudyInput, StudyPlacement};
    use data::{Candle, ChartBasis, Price, Timeframe, Timestamp, Volume};

    fn make_candle(time: u64, close: f32) -> Candle {
        Candle::new(
            Timestamp(time),
            Price::from_f32(close),
            Price::from_f32(close),
            Price::from_f32(close),
            Price::from_f32(close),
            Volume(0.0),
            Volume(0.0),
        )
        .expect("test candle")
    }

    fn make_input(candles: &[Candle]) -> StudyInput<'_> {
        StudyInput {
            candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        }
    }

    #[test]
    fn test_list_by_placement_panel_not_empty() {
        let registry = StudyRegistry::new();
        assert!(!registry.list_by_placement(StudyPlacement::Panel).is_empty());
    }

    #[test]
    fn test_list_by_placement_overlay_not_empty() {
        let registry = StudyRegistry::new();
        assert!(
            !registry
                .list_by_placement(StudyPlacement::Overlay)
                .is_empty()
        );
    }

    #[test]
    fn test_list_by_placement_background_not_empty() {
        let registry = StudyRegistry::new();
        assert!(
            !registry
                .list_by_placement(StudyPlacement::Background)
                .is_empty()
        );
    }

    #[test]
    fn test_list_by_placement_candle_replace_not_empty() {
        let registry = StudyRegistry::new();
        assert!(
            !registry
                .list_by_placement(StudyPlacement::CandleReplace)
                .is_empty()
        );
    }

    #[test]
    fn test_registry_completeness_all_studies_compute() {
        let registry = StudyRegistry::new();
        let candles: Vec<Candle> = (0..50)
            .map(|i| make_candle(i * 60_000, 100.0 + i as f32))
            .collect();
        let input = make_input(&candles);

        for info in registry.list() {
            let mut study = registry
                .create(&info.id)
                .unwrap_or_else(|| panic!("study '{}' not creatable", info.id));
            let result = study.compute(&input);
            assert!(
                result.is_ok(),
                "study '{}' compute() failed: {:?}",
                info.id,
                result.err()
            );
        }
    }

    // ── Registry: contains / create ─────────────────────────

    #[test]
    fn test_registry_contains_known_ids() {
        let registry = StudyRegistry::new();
        let expected = [
            "volume",
            "delta",
            "cvd",
            "obv",
            "sma",
            "ema",
            "vwap",
            "rsi",
            "macd",
            "stochastic",
            "atr",
            "bollinger",
            "imbalance",
            "big_trades",
            "footprint",
            "vbp",
            "speed_of_tape",
            "level_analyzer",
        ];
        for id in &expected {
            assert!(registry.contains(id), "registry missing '{}'", id);
        }
    }

    #[test]
    fn test_registry_contains_unknown_returns_false() {
        let registry = StudyRegistry::new();
        assert!(!registry.contains("nonexistent"));
        assert!(!registry.contains(""));
    }

    #[test]
    fn test_registry_create_unknown_returns_none() {
        let registry = StudyRegistry::new();
        assert!(registry.create("nonexistent").is_none());
    }

    #[test]
    fn test_registry_create_returns_fresh_instances() {
        let registry = StudyRegistry::new();
        let a = registry.create("sma");
        let b = registry.create("sma");
        assert!(a.is_some());
        assert!(b.is_some());
        // Both should be independent instances
    }

    // ── Registry: list ──────────────────────────────────────

    #[test]
    fn test_registry_list_count() {
        let registry = StudyRegistry::new();
        assert_eq!(registry.list().len(), 18);
    }

    #[test]
    fn test_registry_list_sorted_alphabetically() {
        let registry = StudyRegistry::new();
        let names: Vec<String> = registry.list().iter().map(|i| i.name.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    // ── Registry: list_by_category ──────────────────────────

    #[test]
    fn test_registry_list_by_category_volume() {
        let registry = StudyRegistry::new();
        let volume = registry.list_by_category(StudyCategory::Volume);
        assert_eq!(volume.len(), 4);
        let ids: Vec<&str> = volume.iter().map(|i| i.id.as_str()).collect();
        assert!(ids.contains(&"volume"));
        assert!(ids.contains(&"delta"));
        assert!(ids.contains(&"cvd"));
        assert!(ids.contains(&"obv"));
    }

    #[test]
    fn test_registry_list_by_category_trend() {
        let registry = StudyRegistry::new();
        let trend = registry.list_by_category(StudyCategory::Trend);
        assert_eq!(trend.len(), 3);
    }

    #[test]
    fn test_registry_list_by_category_momentum() {
        let registry = StudyRegistry::new();
        let momentum = registry.list_by_category(StudyCategory::Momentum);
        assert_eq!(momentum.len(), 3);
    }

    #[test]
    fn test_registry_list_by_category_volatility() {
        let registry = StudyRegistry::new();
        let vol = registry.list_by_category(StudyCategory::Volatility);
        assert_eq!(vol.len(), 2);
    }

    #[test]
    fn test_registry_list_by_category_orderflow() {
        let registry = StudyRegistry::new();
        let of = registry.list_by_category(StudyCategory::OrderFlow);
        assert_eq!(of.len(), 6);
    }

    #[test]
    fn test_registry_list_by_category_sorted() {
        let registry = StudyRegistry::new();
        let volume = registry.list_by_category(StudyCategory::Volume);
        let names: Vec<String> = volume.iter().map(|i| i.name.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    // ── Registry: list_by_placement ─────────────────────────

    #[test]
    fn test_registry_list_by_placement_overlay_studies() {
        let registry = StudyRegistry::new();
        let overlays = registry.list_by_placement(StudyPlacement::Overlay);
        let ids: Vec<&str> = overlays.iter().map(|i| i.id.as_str()).collect();
        assert!(ids.contains(&"sma"));
        assert!(ids.contains(&"ema"));
        assert!(ids.contains(&"vwap"));
        assert!(ids.contains(&"bollinger"));
        assert!(ids.contains(&"big_trades"));
    }

    #[test]
    fn test_registry_list_by_placement_candle_replace() {
        let registry = StudyRegistry::new();
        let cr = registry.list_by_placement(StudyPlacement::CandleReplace);
        assert_eq!(cr.len(), 1);
        assert_eq!(cr[0].id, "footprint");
    }

    // ── Registry: StudyInfo fields ──────────────────────────

    #[test]
    fn test_study_info_id_matches_key() {
        let registry = StudyRegistry::new();
        for info in registry.list() {
            assert!(
                registry.contains(&info.id),
                "info.id='{}' not found as key in registry",
                info.id,
            );
        }
    }

    #[test]
    fn test_study_info_all_have_descriptions() {
        let registry = StudyRegistry::new();
        for info in registry.list() {
            assert!(
                !info.description.is_empty(),
                "study '{}' has empty description",
                info.id,
            );
        }
    }

    // ── Registry: created study has correct metadata ────────

    #[test]
    fn test_created_study_has_matching_id() {
        let registry = StudyRegistry::new();
        for info in registry.list() {
            let study = registry.create(&info.id).unwrap();
            assert_eq!(study.id(), info.id, "study.id() mismatch for '{}'", info.id,);
        }
    }

    #[test]
    fn test_created_study_has_matching_placement() {
        let registry = StudyRegistry::new();
        for info in registry.list() {
            let study = registry.create(&info.id).unwrap();
            assert_eq!(
                study.placement(),
                info.placement,
                "placement mismatch for '{}'",
                info.id,
            );
        }
    }

    // ── Registry: Default impl ──────────────────────────────

    #[test]
    fn test_registry_default_equals_new() {
        let a = StudyRegistry::new();
        let b = StudyRegistry::default();
        assert_eq!(a.list().len(), b.list().len());
    }

    // ── Registry: custom registration ───────────────────────

    #[test]
    fn test_register_custom_study() {
        let mut registry = StudyRegistry::new();
        let count_before = registry.list().len();
        registry.register(
            "custom_test",
            StudyInfo {
                id: "custom_test".to_string(),
                name: "Custom Test".to_string(),
                category: StudyCategory::Trend,
                placement: StudyPlacement::Overlay,
                description: "test study".to_string(),
            },
            || Box::new(crate::studies::trend::sma::SmaStudy::new()),
        );
        assert!(registry.contains("custom_test"));
        assert_eq!(registry.list().len(), count_before + 1);
    }
}
