//! # Feature Extraction Module
//!
//! This module provides functionality to convert study outputs to model-ready tensors.

pub mod config;
pub mod extractor;

pub use config::{FeatureConfig, FeatureDefinition, FeatureTransform, NormalizationMethod};
pub use extractor::{FeatureExtractor, StudyFeatureExtractor};

/// Result type for feature extraction
pub type FeatureResult<T> = Result<T, FeatureError>;

/// Feature extraction error types
#[derive(Debug, thiserror::Error)]
pub enum FeatureError {
    #[error("Study not found: {0}")]
    StudyNotFound(String),

    #[error("Invalid field path: {0}")]
    InvalidFieldPath(String),

    #[error("Insufficient data: need {need} values, have {have}")]
    InsufficientData { need: usize, have: usize },

    #[error("All values missing")]
    AllValuesMissing,

    #[error("Normalization error: {0}")]
    NormalizationError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Helper function to compute statistics for normalization
pub fn compute_statistics(values: &[f64]) -> Option<(f64, f64)> {
    if values.is_empty() {
        return None;
    }

    let sum: f64 = values.iter().sum();
    let mean = sum / values.len() as f64;

    let variance: f64 =
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;

    let std_dev = variance.sqrt();

    Some((mean, std_dev))
}

/// Helper function to apply z-score normalization
pub fn zscore_normalize(values: &[f64], mean: f64, std_dev: f64) -> Vec<f64> {
    if std_dev == 0.0 {
        return values.iter().map(|_| 0.0).collect();
    }

    values.iter().map(|v| (v - mean) / std_dev).collect()
}

/// Helper function to apply min-max normalization
pub fn minmax_normalize(values: &[f64]) -> Option<Vec<f64>> {
    if values.is_empty() {
        return None;
    }

    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    if (max - min).abs() < f64::EPSILON {
        return Some(values.iter().map(|_| 0.0).collect());
    }

    Some(values.iter().map(|v| (v - min) / (max - min)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_statistics() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (mean, std_dev) = compute_statistics(&values).unwrap();

        assert!((mean - 3.0).abs() < 0.001);
        // Std dev for [1,2,3,4,5] is approximately 1.41
        assert!((std_dev - 1.414).abs() < 0.01);
    }

    #[test]
    fn test_compute_statistics_empty() {
        let values: Vec<f64> = vec![];
        assert!(compute_statistics(&values).is_none());
    }

    #[test]
    fn test_zscore_normalization() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mean = 3.0;
        let std_dev = 1.414;

        let normalized = zscore_normalize(&values, mean, std_dev);

        // Check mean is ~0
        let norm_mean: f64 = normalized.iter().sum::<f64>() / normalized.len() as f64;
        assert!(norm_mean.abs() < 0.001);
    }

    #[test]
    fn test_zscore_normalization_zero_stddev() {
        let values = vec![1.0, 1.0, 1.0, 1.0];
        let normalized = zscore_normalize(&values, 1.0, 0.0);

        // All values should be 0 when std_dev is 0
        assert!(normalized.iter().all(|v| v.abs() < 0.001));
    }

    #[test]
    fn test_minmax_normalization() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let normalized = minmax_normalize(&values).unwrap();

        // Min should be 0, max should be 1
        assert!((normalized[0] - 0.0).abs() < 0.001);
        assert!((normalized[4] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_minmax_normalization_empty() {
        let values: Vec<f64> = vec![];
        assert!(minmax_normalize(&values).is_none());
    }
}
