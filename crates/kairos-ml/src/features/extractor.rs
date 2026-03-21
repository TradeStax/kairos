//! # Feature Extractor
//!
//! Implementation of feature extraction from study outputs.

use super::{FeatureConfig, FeatureError, FeatureResult, NormalizationMethod};
use std::collections::HashMap;

/// Feature extractor trait for converting study outputs to tensors
pub trait FeatureExtractor: Send + Sync {
    /// Add a study output
    fn add_study(&mut self, key: &str, values: &[f64], timestamps: &[i64]);

    /// Extract features as a tensor
    fn extract(&self, lookback: usize) -> FeatureResult<Vec<Vec<f64>>>;

    /// Reset the extractor state
    fn reset(&mut self);

    /// Get the number of features
    fn num_features(&self) -> usize;
}

/// Study output extractor for feature generation
pub struct StudyFeatureExtractor {
    /// Feature configuration
    config: FeatureConfig,
    /// Rolling buffers for each study
    buffers: HashMap<String, Vec<f64>>,
    /// Rolling timestamps
    timestamps: HashMap<String, Vec<i64>>,
}

impl StudyFeatureExtractor {
    /// Create a new extractor from configuration
    pub fn new(config: FeatureConfig) -> Self {
        Self {
            config,
            buffers: HashMap::new(),
            timestamps: HashMap::new(),
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &FeatureConfig {
        &self.config
    }

    /// Add a scalar value for a study (single value per bar)
    pub fn add_scalar(&mut self, key: &str, value: f64, timestamp: i64) {
        let buffer = self.buffers.entry(key.to_string()).or_default();
        let ts = self.timestamps.entry(key.to_string()).or_default();

        buffer.push(value);
        ts.push(timestamp);

        // Limit buffer size to prevent unbounded growth
        let max_size = self.config.lookback_periods * 2;
        if buffer.len() > max_size {
            buffer.remove(0);
            ts.remove(0);
        }
    }

    /// Get the current buffer for a study
    pub fn get_buffer(&self, key: &str) -> Option<&[f64]> {
        self.buffers.get(key).map(|v| v.as_slice())
    }

    /// Get the current values respecting lookback
    fn get_values_for_lookback(&self, key: &str, lookback: usize) -> FeatureResult<Vec<f64>> {
        let buffer = self
            .buffers
            .get(key)
            .ok_or_else(|| FeatureError::StudyNotFound(key.to_string()))?;

        if buffer.len() < lookback {
            return Err(FeatureError::InsufficientData {
                need: lookback,
                have: buffer.len(),
            });
        }

        // Get the last `lookback` values
        Ok(buffer[buffer.len() - lookback..].to_vec())
    }
}

impl FeatureExtractor for StudyFeatureExtractor {
    fn add_study(&mut self, key: &str, values: &[f64], timestamps: &[i64]) {
        assert_eq!(values.len(), timestamps.len());

        let buffer = self.buffers.entry(key.to_string()).or_default();
        let ts = self.timestamps.entry(key.to_string()).or_default();

        buffer.extend(values);
        ts.extend(timestamps);

        // Limit buffer size
        let max_size = self.config.lookback_periods * 2;
        if buffer.len() > max_size {
            let excess = buffer.len() - max_size;
            buffer.drain(0..excess);
            ts.drain(0..excess);
        }
    }

    fn extract(&self, lookback: usize) -> FeatureResult<Vec<Vec<f64>>> {
        let mut result: Vec<Vec<f64>> = Vec::new();

        for feature in &self.config.features {
            let mut values = self.get_values_for_lookback(&feature.study_key, lookback)?;

            // Apply transform if specified
            if let Some(transform) = feature.transform {
                values = transform.apply(&values);
            }

            // Apply normalization
            values = self.normalize(&values)?;

            result.push(values);
        }

        Ok(result)
    }

    fn reset(&mut self) {
        self.buffers.clear();
        self.timestamps.clear();
    }

    fn num_features(&self) -> usize {
        self.config.features.len()
    }
}

impl StudyFeatureExtractor {
    /// Normalize values according to configuration
    fn normalize(&self, values: &[f64]) -> FeatureResult<Vec<f64>> {
        match self.config.normalization {
            NormalizationMethod::None => Ok(values.to_vec()),
            NormalizationMethod::ZScore => {
                let (mean, std_dev) = crate::features::compute_statistics(values)
                    .ok_or(FeatureError::AllValuesMissing)?;
                Ok(crate::features::zscore_normalize(values, mean, std_dev))
            }
            NormalizationMethod::MinMax => {
                crate::features::minmax_normalize(values).ok_or(FeatureError::AllValuesMissing)
            }
        }
    }
}

/// Helper to forward-fill missing values
pub fn forward_fill(values: &[Option<f64>]) -> Vec<f64> {
    let mut result = Vec::with_capacity(values.len());
    let mut last_valid: f64 = 0.0;
    let mut has_valid = false;

    for v in values {
        match v {
            Some(val) => {
                last_valid = *val;
                has_valid = true;
            }
            None if has_valid => {
                // Use last valid value
            }
            None => {
                // No valid value yet
            }
        }
        result.push(last_valid);
    }

    result
}

/// Helper to check if all values are missing
pub fn all_missing(values: &[Option<f64>]) -> bool {
    !values.iter().any(|v| v.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::FeatureDefinition;
    use crate::features::NormalizationMethod;

    fn create_test_config() -> FeatureConfig {
        FeatureConfig {
            features: vec![
                FeatureDefinition::new("sma", "line"),
                FeatureDefinition::new("rsi", "value"),
            ],
            lookback_periods: 5,
            normalization: NormalizationMethod::None,
        }
    }

    #[test]
    fn test_feature_extractor_extracts_single_feature() {
        let config = FeatureConfig {
            features: vec![FeatureDefinition::new("sma", "line")],
            lookback_periods: 3,
            normalization: NormalizationMethod::None,
        };

        let mut extractor = StudyFeatureExtractor::new(config);

        // Add values
        for i in 1..=5 {
            extractor.add_scalar("sma", i as f64, i as i64);
        }

        let result = extractor.extract(3).unwrap();

        assert_eq!(result.len(), 1); // 1 feature
        assert_eq!(result[0].len(), 3); // lookback of 3
        assert_eq!(result[0], vec![3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_feature_extractor_multiple_features() {
        let config = create_test_config();
        let mut extractor = StudyFeatureExtractor::new(config);

        // Add SMA values
        for i in 1..=6 {
            extractor.add_scalar("sma", i as f64, i as i64);
        }

        // Add RSI values
        for i in 1..=6 {
            extractor.add_scalar("rsi", 50.0 + i as f64, i as i64);
        }

        let result = extractor.extract(5).unwrap();

        assert_eq!(result.len(), 2); // 2 features
        assert_eq!(result[0].len(), 5); // lookback of 5
        assert_eq!(result[1].len(), 5);
    }

    #[test]
    fn test_feature_extractor_reset_clears_buffer() {
        let config = create_test_config();
        let mut extractor = StudyFeatureExtractor::new(config);

        extractor.add_scalar("sma", 1.0, 1);
        extractor.reset();

        let result = extractor.extract(3);
        assert!(result.is_err());
    }

    #[test]
    fn test_feature_extractor_lookback_respects_config() {
        let config = FeatureConfig {
            features: vec![FeatureDefinition::new("sma", "line")],
            lookback_periods: 10,
            normalization: NormalizationMethod::None,
        };

        let mut extractor = StudyFeatureExtractor::new(config);

        // Add only 5 values
        for i in 1..=5 {
            extractor.add_scalar("sma", i as f64, i as i64);
        }

        // Requesting lookback of 10 should fail
        let result = extractor.extract(10);
        assert!(result.is_err());
    }

    #[test]
    fn test_forward_fill_handles_missing() {
        let values = vec![Some(1.0), None, Some(3.0), None, None, Some(5.0)];
        let filled = forward_fill(&values);
        assert_eq!(filled, vec![1.0, 1.0, 3.0, 3.0, 3.0, 5.0]);
    }

    #[test]
    fn test_all_missing() {
        let all_none: Vec<Option<f64>> = vec![None, None, None];
        assert!(all_missing(&all_none));

        let some_valid = vec![None, Some(1.0), None];
        assert!(!all_missing(&some_valid));
    }
}
