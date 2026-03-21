//! # Data Generator Module
//!
//! Generates training datasets from historical candle and study data.

use super::{Dataset, LabelConfig, TrainingConfig};
use crate::features::{FeatureConfig, FeatureExtractor, StudyFeatureExtractor};
use serde::{Deserialize, Serialize};

/// Candle data structure for generating datasets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    /// Opening price
    pub open: f64,
    /// High price
    pub high: f64,
    /// Low price
    pub low: f64,
    /// Closing price
    pub close: f64,
    /// Volume
    pub volume: f64,
    /// Timestamp (milliseconds since epoch)
    pub timestamp: i64,
}

impl Candle {
    /// Create a new candle
    pub fn new(open: f64, high: f64, low: f64, close: f64, volume: f64, timestamp: i64) -> Self {
        Self {
            open,
            high,
            low,
            close,
            volume,
            timestamp,
        }
    }

    /// Get the return of this candle
    pub fn returns(&self) -> f64 {
        if self.open.abs() < f64::EPSILON {
            0.0
        } else {
            (self.close - self.open) / self.open
        }
    }

    /// Get N-bar forward return
    pub fn forward_return(&self, future_close: f64) -> f64 {
        if self.close.abs() < f64::EPSILON {
            0.0
        } else {
            (future_close - self.close) / self.close
        }
    }
}

/// Study output for dataset generation
#[derive(Debug, Clone)]
pub struct StudyOutput {
    /// Study values
    pub values: Vec<f64>,
    /// Timestamps matching the values
    pub timestamps: Vec<i64>,
}

impl StudyOutput {
    /// Create a new study output
    pub fn new(values: Vec<f64>, timestamps: Vec<i64>) -> Self {
        assert_eq!(values.len(), timestamps.len());
        Self { values, timestamps }
    }
}

/// Data generator for creating training datasets
pub struct DataGenerator {
    /// Feature configuration
    feature_config: FeatureConfig,
    /// Label configuration
    label_config: LabelConfig,
    /// Feature extractor
    extractor: StudyFeatureExtractor,
}

impl DataGenerator {
    /// Create a new data generator
    pub fn new(feature_config: FeatureConfig, label_config: LabelConfig) -> Self {
        Self {
            feature_config: feature_config.clone(),
            label_config,
            extractor: StudyFeatureExtractor::new(feature_config),
        }
    }

    /// Create from training config
    pub fn from_config(config: &TrainingConfig) -> Self {
        Self::new(
            config.label_config.clone().into(),
            config.label_config.clone(),
        )
    }

    /// Generate a dataset from candles and study outputs
    pub fn generate(
        &mut self,
        candles: &[Candle],
        studies: &[(&str, StudyOutput)],
    ) -> Result<Dataset, DataGeneratorError> {
        let lookback = self.feature_config.lookback_periods;
        let horizon = self.label_config.horizon;
        let warmup = self.label_config.warmup_bars;

        // Validate we have enough data
        if candles.len() < lookback + horizon + warmup {
            return Err(DataGeneratorError::InsufficientData {
                need: lookback + horizon + warmup,
                have: candles.len(),
            });
        }

        // Reset extractor for fresh start
        self.extractor.reset();

        // Add all study data to extractor
        for (key, study) in studies {
            self.extractor
                .add_study(key, &study.values, &study.timestamps);
        }

        // Calculate how many samples we can generate
        let usable_bars = candles.len() - warmup - horizon;
        let num_samples = usable_bars.saturating_sub(lookback);

        if num_samples == 0 {
            return Err(DataGeneratorError::InsufficientData {
                need: lookback + horizon + warmup + 1,
                have: candles.len(),
            });
        }

        // Collect features and labels
        let mut all_features: Vec<Vec<Vec<f64>>> = Vec::with_capacity(num_samples);
        let mut all_labels: Vec<usize> = Vec::with_capacity(num_samples);
        let mut all_timestamps: Vec<i64> = Vec::with_capacity(num_samples);

        // Compute forward returns for labeling
        let forward_returns = self.compute_forward_returns(candles);

        // Generate samples
        for i in warmup..(warmup + num_samples) {
            // Extract features for this sample
            // We need to extract features at bar i-1 (the bar before the label)
            let extract_idx = i.saturating_sub(1);

            match self.extractor.extract(lookback) {
                Ok(features) => {
                    // Transpose features: [num_features, lookback] -> [lookback, num_features]
                    let transposed = Self::transpose_features(&features);
                    all_features.push(transposed);
                    all_timestamps.push(candles[extract_idx].timestamp);
                }
                Err(e) => {
                    return Err(DataGeneratorError::FeatureExtractionError(e.to_string()));
                }
            }

            // Generate label from forward return
            let label = Self::generate_label(
                forward_returns.get(extract_idx).copied().unwrap_or(0.0),
                &self.label_config,
            );
            all_labels.push(label);
        }

        Ok(Dataset::new(all_features, all_labels, all_timestamps))
    }

    /// Compute forward returns for all bars
    fn compute_forward_returns(&self, candles: &[Candle]) -> Vec<f64> {
        let horizon = self.label_config.horizon;

        candles
            .windows(horizon + 1)
            .map(|window| {
                if window.len() < 2 {
                    0.0
                } else {
                    let current = &window[0];
                    let future = &window[window.len() - 1];
                    current.forward_return(future.close)
                }
            })
            .collect()
    }

    /// Generate a label from return
    fn generate_label(returns: f64, config: &LabelConfig) -> usize {
        if returns > config.long_threshold {
            0 // Long
        } else if returns < -config.short_threshold {
            2 // Short
        } else {
            1 // Neutral
        }
    }

    /// Transpose features from [features, lookback] to [lookback, features]
    fn transpose_features(features: &[Vec<f64>]) -> Vec<Vec<f64>> {
        if features.is_empty() {
            return vec![];
        }

        let num_features = features.len();
        let lookback = features[0].len();

        let mut transposed = vec![vec![0.0; num_features]; lookback];

        for (feature_idx, feature_values) in features.iter().enumerate() {
            for (time_idx, value) in feature_values.iter().enumerate() {
                transposed[time_idx][feature_idx] = *value;
            }
        }

        transposed
    }
}

impl From<LabelConfig> for FeatureConfig {
    fn from(_config: LabelConfig) -> Self {
        FeatureConfig::default()
    }
}

impl From<LabelConfig> for DataGenerator {
    fn from(config: LabelConfig) -> Self {
        Self::new(FeatureConfig::default(), config)
    }
}

/// Data generator errors
#[derive(Debug, thiserror::Error)]
pub enum DataGeneratorError {
    #[error("Insufficient data: need {need} bars, have {have}")]
    InsufficientData { need: usize, have: usize },

    #[error("Feature extraction error: {0}")]
    FeatureExtractionError(String),

    #[error("Invalid candle data: {0}")]
    InvalidCandleData(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::FeatureDefinition;
    use crate::features::NormalizationMethod;

    fn create_test_candles(count: usize) -> Vec<Candle> {
        let base_price = 100.0;
        (0..count)
            .map(|i| {
                let open = base_price + (i as f64 * 0.1);
                let close = open + (i as f64 % 3.0 - 1.0) * 0.5;
                Candle::new(
                    open,
                    open.max(close) + 0.2,
                    open.min(close) - 0.1,
                    close,
                    1000.0,
                    i as i64 * 60000, // 1 minute bars
                )
            })
            .collect()
    }

    fn create_test_studies(candle_count: usize) -> Vec<(&'static str, StudyOutput)> {
        let sma_values: Vec<f64> = (0..candle_count)
            .map(|i| 100.0 + (i as f64 * 0.1) + 0.5)
            .collect();
        let timestamps: Vec<i64> = (0..candle_count).map(|i| i as i64 * 60000).collect();

        let rsi_values: Vec<f64> = (0..candle_count)
            .map(|i| 50.0 + ((i % 10) as f64 - 5.0) * 5.0)
            .collect();

        vec![
            ("sma", StudyOutput::new(sma_values, timestamps.clone())),
            ("rsi", StudyOutput::new(rsi_values, timestamps)),
        ]
    }

    fn create_feature_config() -> FeatureConfig {
        FeatureConfig {
            features: vec![
                FeatureDefinition::new("sma", "line"),
                FeatureDefinition::new("rsi", "value"),
            ],
            lookback_periods: 5,
            normalization: NormalizationMethod::None,
        }
    }

    fn create_label_config() -> LabelConfig {
        LabelConfig {
            horizon: 1,
            long_threshold: 0.005,
            short_threshold: 0.005,
            warmup_bars: 10,
        }
    }

    #[test]
    fn test_generate_dataset_from_candles() {
        let candles = create_test_candles(50);
        let studies = create_test_studies(50);

        let feature_config = create_feature_config();
        let label_config = create_label_config();

        let mut generator = DataGenerator::new(feature_config, label_config);
        let dataset = generator.generate(&candles, &studies);

        assert!(dataset.is_ok());
        let dataset = dataset.unwrap();

        // Should have samples based on: total - warmup - horizon - lookback
        // 50 - 10 - 1 - 5 = 34 samples
        assert!(dataset.len() >= 30);
        assert_eq!(dataset.num_features(), 2);
        assert_eq!(dataset.lookback(), 5);
    }

    #[test]
    fn test_generate_dataset_handles_insufficient_data() {
        let candles = create_test_candles(5); // Too few candles
        let studies = create_test_studies(5);

        let feature_config = create_feature_config();
        let label_config = create_label_config();

        let mut generator = DataGenerator::new(feature_config, label_config);
        let result = generator.generate(&candles, &studies);

        assert!(result.is_err());
        match result {
            Err(DataGeneratorError::InsufficientData { need, have }) => {
                assert!(need > have);
            }
            _ => panic!("Expected InsufficientData error"),
        }
    }

    #[test]
    fn test_forward_return_calculation() {
        let candle = Candle::new(100.0, 101.0, 99.0, 101.0, 1000.0, 0);
        let future_close = 102.0;

        let forward_return = candle.forward_return(future_close);
        assert!((forward_return - 0.01).abs() < 0.001); // 1% return
    }

    #[test]
    fn test_label_generation_thresholds() {
        let config = LabelConfig {
            horizon: 1,
            long_threshold: 0.005,
            short_threshold: 0.005,
            warmup_bars: 10,
        };

        // Test long
        assert_eq!(DataGenerator::generate_label(0.01, &config), 0);

        // Test neutral
        assert_eq!(DataGenerator::generate_label(0.002, &config), 1);
        assert_eq!(DataGenerator::generate_label(0.0, &config), 1);
        assert_eq!(DataGenerator::generate_label(-0.002, &config), 1);

        // Test short
        assert_eq!(DataGenerator::generate_label(-0.01, &config), 2);
    }

    #[test]
    fn test_transpose_features() {
        let features = vec![
            vec![1.0, 2.0, 3.0], // feature 0
            vec![4.0, 5.0, 6.0], // feature 1
        ];

        let transposed = DataGenerator::transpose_features(&features);

        assert_eq!(transposed.len(), 3); // lookback
        assert_eq!(transposed[0], vec![1.0, 4.0]);
        assert_eq!(transposed[1], vec![2.0, 5.0]);
        assert_eq!(transposed[2], vec![3.0, 6.0]);
    }

    #[test]
    fn test_candle_returns() {
        let candle = Candle::new(100.0, 102.0, 99.0, 101.0, 1000.0, 0);
        assert!((candle.returns() - 0.01).abs() < 0.001);
    }

    #[test]
    fn test_data_generator_from_config() {
        let training_config = TrainingConfig::default();
        let generator = DataGenerator::from_config(&training_config);

        // Should be created with default configs
        assert_eq!(generator.extractor.num_features(), 0); // No features defined
    }
}
