//! # ML Strategy Configuration
//!
//! Configuration types for the ML strategy wrapper.

use crate::features::FeatureConfig;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stop-loss and take-profit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopLossTakeProfitConfig {
    /// Stop-loss distance in ticks (0 = disabled)
    #[serde(default)]
    pub stop_loss_ticks: i32,
    /// Take-profit distance in ticks (0 = disabled)
    #[serde(default)]
    pub take_profit_ticks: i32,
    /// Use ATR-based SL/TP instead of fixed ticks
    #[serde(default)]
    pub use_atr_based: bool,
    /// ATR multiplier for stop-loss (only used when use_atr_based = true)
    #[serde(default = "default_atr_multiplier")]
    pub stop_loss_atr_multiplier: f64,
    /// ATR multiplier for take-profit (only used when use_atr_based = true)
    #[serde(default = "default_atr_multiplier")]
    pub take_profit_atr_multiplier: f64,
}

fn default_atr_multiplier() -> f64 {
    2.0
}

impl Default for StopLossTakeProfitConfig {
    fn default() -> Self {
        Self {
            stop_loss_ticks: 0,
            take_profit_ticks: 0,
            use_atr_based: false,
            stop_loss_atr_multiplier: 2.0,
            take_profit_atr_multiplier: 2.0,
        }
    }
}

impl StopLossTakeProfitConfig {
    /// Create a new SL/TP config with fixed ticks
    pub fn fixed_ticks(sl_ticks: i32, tp_ticks: i32) -> Self {
        Self {
            stop_loss_ticks: sl_ticks,
            take_profit_ticks: tp_ticks,
            use_atr_based: false,
            stop_loss_atr_multiplier: 2.0,
            take_profit_atr_multiplier: 2.0,
        }
    }

    /// Create a new SL/TP config with ATR-based distances
    pub fn atr_based(sl_multiplier: f64, tp_multiplier: f64) -> Self {
        Self {
            stop_loss_ticks: 0,
            take_profit_ticks: 0,
            use_atr_based: true,
            stop_loss_atr_multiplier: sl_multiplier,
            take_profit_atr_multiplier: tp_multiplier,
        }
    }

    /// Check if stop-loss is enabled
    pub fn has_stop_loss(&self) -> bool {
        self.stop_loss_ticks > 0 || self.use_atr_based
    }

    /// Check if take-profit is enabled
    pub fn has_take_profit(&self) -> bool {
        self.take_profit_ticks > 0 || self.use_atr_based
    }
}

/// Configuration for the ML strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlStrategyConfig {
    /// Optional unique identifier (defaults to "ml_strategy")
    pub id: Option<String>,
    /// Strategy name for display
    pub name: Option<String>,
    /// Strategy description
    pub description: Option<String>,
    /// Path to the trained model file (ONNX or PyTorch)
    pub model_path: Option<String>,
    /// Feature extraction configuration
    pub feature_config: FeatureConfig,
    /// Probability threshold for long signals
    #[serde(default = "default_signal_threshold")]
    pub signal_threshold_long: f64,
    /// Probability threshold for short signals
    #[serde(default = "default_signal_threshold")]
    pub signal_threshold_short: f64,
    /// Minimum confidence to trigger an order
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f64,
    /// Use model confidence for position sizing
    #[serde(default)]
    pub use_confidence_for_sizing: bool,
    /// Stop-loss and take-profit configuration
    #[serde(default)]
    pub sl_tp: Option<StopLossTakeProfitConfig>,
}

fn default_signal_threshold() -> f64 {
    0.6
}

fn default_min_confidence() -> f64 {
    0.5
}

impl MlStrategyConfig {
    /// Create a new ML strategy config
    pub fn new(feature_config: FeatureConfig) -> Self {
        Self {
            id: None,
            name: None,
            description: None,
            model_path: None,
            feature_config,
            signal_threshold_long: 0.6,
            signal_threshold_short: 0.6,
            min_confidence: 0.5,
            use_confidence_for_sizing: false,
            sl_tp: None,
        }
    }

    /// Set stop-loss and take-profit config
    pub fn with_sl_tp(mut self, sl_tp: StopLossTakeProfitConfig) -> Self {
        self.sl_tp = Some(sl_tp);
        self
    }

    /// Set the model path
    pub fn model_path(mut self, path: impl Into<String>) -> Self {
        self.model_path = Some(path.into());
        self
    }

    /// Set the strategy ID
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the signal thresholds
    pub fn signal_thresholds(mut self, long: f64, short: f64) -> Self {
        self.signal_threshold_long = long;
        self.signal_threshold_short = short;
        self
    }

    /// Set minimum confidence
    pub fn min_confidence(mut self, confidence: f64) -> Self {
        self.min_confidence = confidence;
        self
    }

    /// Set the feature configuration
    pub fn with_feature_config(mut self, feature_config: FeatureConfig) -> Self {
        self.feature_config = feature_config;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), MlStrategyConfigError> {
        // Validate feature config
        self.feature_config
            .validate()
            .map_err(|e| MlStrategyConfigError::InvalidFeatureConfig(e.to_string()))?;

        // Validate thresholds
        if !(0.0..=1.0).contains(&self.signal_threshold_long) {
            return Err(MlStrategyConfigError::InvalidThreshold {
                field: "signal_threshold_long".to_string(),
                value: self.signal_threshold_long,
                message: "must be between 0.0 and 1.0".to_string(),
            });
        }

        if !(0.0..=1.0).contains(&self.signal_threshold_short) {
            return Err(MlStrategyConfigError::InvalidThreshold {
                field: "signal_threshold_short".to_string(),
                value: self.signal_threshold_short,
                message: "must be between 0.0 and 1.0".to_string(),
            });
        }

        if !(0.0..=1.0).contains(&self.min_confidence) {
            return Err(MlStrategyConfigError::InvalidThreshold {
                field: "min_confidence".to_string(),
                value: self.min_confidence,
                message: "must be between 0.0 and 1.0".to_string(),
            });
        }

        Ok(())
    }
}

impl Default for MlStrategyConfig {
    fn default() -> Self {
        Self {
            id: None,
            name: None,
            description: None,
            model_path: None,
            feature_config: FeatureConfig::default(),
            signal_threshold_long: 0.6,
            signal_threshold_short: 0.6,
            min_confidence: 0.5,
            use_confidence_for_sizing: false,
            sl_tp: None,
        }
    }
}

impl fmt::Display for MlStrategyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MlStrategyConfig(")?;
        if let Some(ref id) = self.id {
            write!(f, "id={}, ", id)?;
        }
        write!(f, "features={}, ", self.feature_config.features.len())?;
        write!(f, "lookback={}", self.feature_config.lookback_periods)?;
        write!(f, ")")
    }
}

/// Errors for ML strategy configuration
#[derive(Debug, thiserror::Error)]
pub enum MlStrategyConfigError {
    #[error("invalid feature config: {0}")]
    InvalidFeatureConfig(String),

    #[error("invalid threshold for '{field}': {value} - {message}")]
    InvalidThreshold {
        field: String,
        value: f64,
        message: String,
    },

    #[error("model not found: {0}")]
    ModelNotFound(String),

    #[error("feature config validation failed: {0}")]
    FeatureConfigValidation(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{FeatureDefinition, NormalizationMethod};

    #[test]
    fn test_ml_strategy_config_defaults() {
        let config = MlStrategyConfig::default();
        assert_eq!(config.signal_threshold_long, 0.6);
        assert_eq!(config.signal_threshold_short, 0.6);
        assert_eq!(config.min_confidence, 0.5);
        assert!(!config.use_confidence_for_sizing);
    }

    #[test]
    fn test_ml_strategy_config_builder() {
        let config = MlStrategyConfig::new(FeatureConfig::default())
            .id("my_strategy")
            .model_path("/path/to/model.pt")
            .signal_thresholds(0.7, 0.7)
            .min_confidence(0.6);

        assert_eq!(config.id, Some("my_strategy".to_string()));
        assert_eq!(config.model_path, Some("/path/to/model.pt".to_string()));
        assert_eq!(config.signal_threshold_long, 0.7);
        assert_eq!(config.signal_threshold_short, 0.7);
        assert_eq!(config.min_confidence, 0.6);
    }

    #[test]
    fn test_config_validation_rejects_invalid_threshold() {
        // Create config with valid features
        let mut config = MlStrategyConfig::new(FeatureConfig {
            features: vec![FeatureDefinition::new("sma", "line")],
            lookback_periods: 20,
            normalization: NormalizationMethod::ZScore,
        });

        // Invalid threshold (> 1.0)
        config.signal_threshold_long = 1.5;
        assert!(config.validate().is_err());

        // Invalid threshold (< 0.0)
        config.signal_threshold_long = -0.1;
        assert!(config.validate().is_err());

        // Valid threshold
        config.signal_threshold_long = 0.6;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_with_valid_features() {
        let config = MlStrategyConfig::new(FeatureConfig {
            features: vec![FeatureDefinition::new("sma", "line")],
            lookback_periods: 20,
            normalization: NormalizationMethod::ZScore,
        });

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_display() {
        let config = MlStrategyConfig::default();
        let display = format!("{}", config);
        assert!(display.contains("MlStrategyConfig"));
    }
}
