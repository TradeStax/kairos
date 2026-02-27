//! Study (indicator) management for strategy execution.
//!
//! [`StudyBank`] owns study instances declared by a strategy and
//! recomputes their outputs when new candle data arrives. Strategies
//! access computed outputs through
//! [`StrategyContext::studies`](super::context::StrategyContext::studies).

use crate::strategy::StudyRequest;
use kairos_data::{Candle, ChartBasis, Price, Timeframe, Trade};
use kairos_study::{Study, StudyInput, StudyOutput, StudyRegistry};
use std::collections::HashMap;

/// Manages study (indicator) instances for a strategy.
///
/// The engine initializes the bank from the strategy's
/// [`required_studies`](super::Strategy::required_studies) before the
/// run starts, then calls [`recompute`](StudyBank::recompute) after
/// each candle close.
pub struct StudyBank {
    /// Study instances keyed by the strategy-assigned key.
    studies: HashMap<String, Box<dyn Study>>,
    /// Most recent output for each study, keyed by study key.
    outputs: HashMap<String, StudyOutput>,
}

impl StudyBank {
    /// Creates an empty study bank with no studies.
    #[must_use]
    pub fn new() -> Self {
        Self {
            studies: HashMap::new(),
            outputs: HashMap::new(),
        }
    }

    /// Creates and configures study instances from the strategy's
    /// requests using the given registry.
    ///
    /// Unknown study IDs are logged as warnings and skipped.
    pub fn initialize(&mut self, requests: &[StudyRequest], registry: &StudyRegistry) {
        for req in requests {
            if let Some(mut study) = registry.create(&req.study_id) {
                for (key, value) in &req.params {
                    let _ = study.set_parameter(key, value.clone());
                }
                self.studies.insert(req.key.clone(), study);
            } else {
                log::warn!("Study '{}' not found in registry", req.study_id);
            }
        }
    }

    /// Recomputes all studies with the latest candle data.
    ///
    /// Called by the engine after each candle close. Studies that
    /// fail to compute are silently skipped (their previous output
    /// is retained).
    pub fn recompute(
        &mut self,
        candles: &[Candle],
        trades: Option<&[Trade]>,
        tick_size: Price,
        timeframe: Timeframe,
    ) {
        for (key, study) in &mut self.studies {
            let input = StudyInput {
                candles,
                trades,
                basis: ChartBasis::Time(timeframe),
                tick_size,
                visible_range: None,
            };
            if study.compute(&input).is_ok() {
                self.outputs.insert(key.clone(), study.output().clone());
            }
        }
    }

    /// Returns the most recent output for the study with the given
    /// key, or `None` if the key does not exist or has not yet been
    /// computed.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&StudyOutput> {
        self.outputs.get(key)
    }

    /// Returns `true` if a study with the given key is registered.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.studies.contains_key(key)
    }

    /// Resets all studies and clears cached outputs.
    ///
    /// Called between optimization runs to restore clean state.
    pub fn reset(&mut self) {
        for study in self.studies.values_mut() {
            study.reset();
        }
        self.outputs.clear();
    }
}

impl Default for StudyBank {
    fn default() -> Self {
        Self::new()
    }
}
