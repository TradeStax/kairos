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
pub use orderflow::{BigTradesStudy, FootprintStudy, ImbalanceStudy, VbpStudy};
pub use trend::{EmaStudy, SmaStudy, VwapStudy};
pub use volatility::{AtrStudy, BollingerStudy};
pub use volume::{CvdStudy, DeltaStudy, ObvStudy, VolumeStudy};

use crate::core::{Study, StudyCategory, StudyPlacement};
use std::collections::HashMap;

/// Metadata about a registered study (used for catalog display).
#[derive(Debug, Clone)]
pub struct StudyInfo {
    pub id: String,
    pub name: String,
    pub category: StudyCategory,
    pub placement: StudyPlacement,
    pub description: String,
}

/// Factory registry for creating study instances by string ID.
///
/// Pre-loaded with all built-in studies on construction. Custom studies can be
/// added via [`register()`](Self::register).
pub struct StudyRegistry {
    factories: HashMap<String, Box<dyn Fn() -> Box<dyn Study> + Send + Sync>>,
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

    /// List studies filtered by placement.
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
}
