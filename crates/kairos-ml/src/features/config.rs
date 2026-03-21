//! # Feature Configuration
//!
//! Configuration types for feature extraction pipeline.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Normalization method for features
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum NormalizationMethod {
    /// Z-score normalization (mean=0, std=1)
    #[default]
    ZScore,
    /// Min-max normalization (range [0, 1])
    MinMax,
    /// No normalization
    None,
}

impl fmt::Display for NormalizationMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NormalizationMethod::ZScore => write!(f, "zscore"),
            NormalizationMethod::MinMax => write!(f, "minmax"),
            NormalizationMethod::None => write!(f, "none"),
        }
    }
}

/// Transform to apply to feature values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum FeatureTransform {
    /// Log transform
    Log,
    /// Difference (first derivative)
    Diff,
    /// Percentage change
    PctChange,
    /// No transform
    #[default]
    None,
}

impl FeatureTransform {
    /// Apply the transform to a slice of values
    pub fn apply(&self, values: &[f64]) -> Vec<f64> {
        match self {
            FeatureTransform::None => values.to_vec(),
            FeatureTransform::Log => values
                .iter()
                .map(|v| v.log2().clamp(-100.0, 100.0))
                .collect(),
            FeatureTransform::Diff => {
                if values.len() < 2 {
                    return vec![0.0; values.len()];
                }
                let mut result = vec![0.0];
                for i in 1..values.len() {
                    result.push(values[i] - values[i - 1]);
                }
                result
            }
            FeatureTransform::PctChange => {
                if values.len() < 2 {
                    return vec![0.0; values.len()];
                }
                let mut result = vec![0.0];
                for i in 1..values.len() {
                    if values[i - 1].abs() < f64::EPSILON {
                        result.push(0.0);
                    } else {
                        result.push((values[i] - values[i - 1]) / values[i - 1]);
                    }
                }
                result
            }
        }
    }
}

impl fmt::Display for FeatureTransform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeatureTransform::Log => write!(f, "log"),
            FeatureTransform::Diff => write!(f, "diff"),
            FeatureTransform::PctChange => write!(f, "pct_change"),
            FeatureTransform::None => write!(f, "none"),
        }
    }
}

/// Definition of a single feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDefinition {
    /// Strategy study key (e.g., "sma_20")
    pub study_key: String,
    /// Which output field to use (e.g., "line", "band.upper")
    pub output_field: String,
    /// Optional transform to apply to values
    #[serde(default)]
    pub transform: Option<FeatureTransform>,
    /// Optional name for the feature (defaults to output_field)
    #[serde(default)]
    pub name: Option<String>,
}

impl FeatureDefinition {
    /// Create a new feature definition
    pub fn new(study_key: &str, output_field: &str) -> Self {
        Self {
            study_key: study_key.to_string(),
            output_field: output_field.to_string(),
            transform: None,
            name: None,
        }
    }

    /// Set the transform
    pub fn with_transform(mut self, transform: FeatureTransform) -> Self {
        self.transform = Some(transform);
        self
    }

    /// Set the name
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Get the display name
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.output_field)
    }
}

/// Feature extraction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Studies to extract as features
    pub features: Vec<FeatureDefinition>,
    /// Number of historical bars to include (lookback)
    pub lookback_periods: usize,
    /// Normalization method
    #[serde(default)]
    pub normalization: NormalizationMethod,
}

impl FeatureConfig {
    /// Create a new feature config
    pub fn new(features: Vec<FeatureDefinition>, lookback_periods: usize) -> Self {
        Self {
            features,
            lookback_periods,
            normalization: NormalizationMethod::ZScore,
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), super::FeatureError> {
        if self.lookback_periods == 0 {
            return Err(super::FeatureError::InvalidConfig(
                "lookback_periods must be > 0".to_string(),
            ));
        }

        if self.features.is_empty() {
            return Err(super::FeatureError::InvalidConfig(
                "at least one feature must be defined".to_string(),
            ));
        }

        for feature in &self.features {
            if feature.study_key.is_empty() {
                return Err(super::FeatureError::InvalidConfig(
                    "study_key cannot be empty".to_string(),
                ));
            }
            if feature.output_field.is_empty() {
                return Err(super::FeatureError::InvalidConfig(
                    "output_field cannot be empty".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Get list of required study keys
    pub fn required_studies(&self) -> Vec<&str> {
        self.features.iter().map(|f| f.study_key.as_str()).collect()
    }
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            features: vec![],
            lookback_periods: 20,
            normalization: NormalizationMethod::ZScore,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_config_defaults() {
        let config = FeatureConfig::default();
        assert_eq!(config.lookback_periods, 20);
        assert!(config.features.is_empty());
        assert_eq!(config.normalization, NormalizationMethod::ZScore);
    }

    #[test]
    fn test_feature_config_validation() {
        let mut config = FeatureConfig::default();

        // Empty features should fail
        assert!(config.validate().is_err());

        // Add a feature
        config.features.push(FeatureDefinition::new("sma", "line"));
        assert!(config.validate().is_ok());

        // Zero lookback should fail
        config.lookback_periods = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_feature_config_serializes_to_json() {
        let config = FeatureConfig {
            features: vec![FeatureDefinition {
                study_key: "sma_20".into(),
                output_field: "line".into(),
                transform: Some(FeatureTransform::PctChange),
                name: None,
            }],
            lookback_periods: 20,
            normalization: NormalizationMethod::ZScore,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("sma_20"));

        let parsed: FeatureConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.lookback_periods, 20);
        assert_eq!(parsed.features.len(), 1);
    }

    #[test]
    fn test_feature_definition_defaults() {
        let def = FeatureDefinition::new("sma", "line");

        assert_eq!(def.study_key, "sma");
        assert_eq!(def.output_field, "line");
        assert!(def.transform.is_none());
        assert_eq!(def.display_name(), "line");
    }

    #[test]
    fn test_feature_definition_chain() {
        let def = FeatureDefinition::new("rsi", "value")
            .with_transform(FeatureTransform::Diff)
            .with_name("rsi_diff");

        assert_eq!(def.study_key, "rsi");
        assert_eq!(def.display_name(), "rsi_diff");
        assert_eq!(def.transform, Some(FeatureTransform::Diff));
    }

    #[test]
    fn test_normalization_method_variants() {
        assert_eq!(
            serde_json::to_string(&NormalizationMethod::ZScore).unwrap(),
            "\"z_score\""
        );
        assert_eq!(
            serde_json::to_string(&NormalizationMethod::MinMax).unwrap(),
            "\"min_max\""
        );
    }

    #[test]
    fn test_feature_transform_apply() {
        // Test Diff transform
        let values = vec![1.0, 2.0, 4.0, 7.0];
        let diff = FeatureTransform::Diff.apply(&values);
        assert_eq!(diff, vec![0.0, 1.0, 2.0, 3.0]);

        // Test PctChange transform
        let pct = FeatureTransform::PctChange.apply(&values);
        assert_eq!(pct[0], 0.0); // First is always 0
        assert!((pct[1] - 1.0).abs() < 0.001); // (2-1)/1 = 1
    }
}
