use crate::strategy::StudyRequest;
use kairos_data::{Candle, ChartBasis, Price, Timeframe, Trade};
use kairos_study::{Study, StudyInput, StudyOutput, StudyRegistry};
use std::collections::HashMap;

/// Manages study (indicator) instances for a strategy.
pub struct StudyBank {
    /// study_key -> study instance
    studies: HashMap<String, Box<dyn Study>>,
    /// study_key -> latest output
    outputs: HashMap<String, StudyOutput>,
}

impl StudyBank {
    pub fn new() -> Self {
        Self {
            studies: HashMap::new(),
            outputs: HashMap::new(),
        }
    }

    /// Initialize studies from strategy requests using the
    /// registry.
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

    /// Recompute all studies with updated candle data.
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

    /// Get the output for a study by key.
    pub fn get(&self, key: &str) -> Option<&StudyOutput> {
        self.outputs.get(key)
    }

    /// Check if a study key exists.
    pub fn contains(&self, key: &str) -> bool {
        self.studies.contains_key(key)
    }

    /// Reset all studies and clear outputs.
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
