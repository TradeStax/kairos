//! # Training Configuration
//!
//! Configuration types for model training.

use super::OptimizerType;
use serde::{Deserialize, Serialize};

/// Training configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Model architecture
    pub model_type: ModelType,
    /// Learning rate
    pub learning_rate: f64,
    /// Batch size
    pub batch_size: usize,
    /// Number of epochs
    pub epochs: usize,
    /// Optimizer type
    pub optimizer: OptimizerType,
    /// Weight decay for regularization
    pub weight_decay: f64,
    /// Label generation
    pub label_config: LabelConfig,
    /// Validation split ratio (0.0 to 1.0)
    pub validation_split: f64,
    /// Early stopping patience (0 to disable)
    pub early_stopping_patience: usize,
}

impl TrainingConfig {
    /// Create a new training config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), TrainingConfigError> {
        if self.learning_rate <= 0.0 {
            return Err(TrainingConfigError::InvalidLearningRate(self.learning_rate));
        }
        if self.batch_size == 0 {
            return Err(TrainingConfigError::InvalidBatchSize(self.batch_size));
        }
        if self.epochs == 0 {
            return Err(TrainingConfigError::InvalidEpochs(self.epochs));
        }
        if !(0.0..=1.0).contains(&self.validation_split) {
            return Err(TrainingConfigError::InvalidValidationSplit(
                self.validation_split,
            ));
        }
        if self.early_stopping_patience > 0 && self.validation_split == 0.0 {
            return Err(TrainingConfigError::EarlyStoppingWithoutValidation);
        }
        Ok(())
    }
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            model_type: ModelType::Mlp,
            learning_rate: 0.001,
            batch_size: 32,
            epochs: 100,
            optimizer: OptimizerType::Adam,
            weight_decay: 0.0001,
            label_config: LabelConfig::default(),
            validation_split: 0.2,
            early_stopping_patience: 10,
        }
    }
}

/// Training configuration errors
#[derive(Debug, thiserror::Error)]
pub enum TrainingConfigError {
    #[error("Learning rate must be positive, got {0}")]
    InvalidLearningRate(f64),
    #[error("Batch size must be positive, got {0}")]
    InvalidBatchSize(usize),
    #[error("Epochs must be positive, got {0}")]
    InvalidEpochs(usize),
    #[error("Validation split must be between 0 and 1, got {0}")]
    InvalidValidationSplit(f64),
    #[error("Early stopping requires validation split > 0")]
    EarlyStoppingWithoutValidation,
}

/// Model architecture type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ModelType {
    /// Simple Multi-Layer Perceptron
    #[default]
    Mlp,
    /// LSTM-based model
    LSTM,
    /// 1D Convolutional model
    Conv1D,
}

/// Label generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelConfig {
    /// Number of bars forward to predict
    pub horizon: usize,
    /// Return threshold for long signal
    pub long_threshold: f64,
    /// Return threshold for short signal (absolute value)
    pub short_threshold: f64,
    /// Minimum bars required before first label
    pub warmup_bars: usize,
}

impl Default for LabelConfig {
    fn default() -> Self {
        Self {
            horizon: 1,
            long_threshold: 0.005,
            short_threshold: 0.005,
            warmup_bars: 20,
        }
    }
}

impl LabelConfig {
    /// Create a new label config
    pub fn new(horizon: usize, long_threshold: f64, short_threshold: f64) -> Self {
        Self {
            horizon,
            long_threshold,
            short_threshold,
            warmup_bars: 20,
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), LabelConfigError> {
        if self.horizon == 0 {
            return Err(LabelConfigError::InvalidHorizon(self.horizon));
        }
        if self.long_threshold < 0.0 {
            return Err(LabelConfigError::NegativeThreshold(self.long_threshold));
        }
        if self.short_threshold < 0.0 {
            return Err(LabelConfigError::NegativeThreshold(self.short_threshold));
        }
        Ok(())
    }
}

/// Label configuration errors
#[derive(Debug, thiserror::Error)]
pub enum LabelConfigError {
    #[error("Horizon must be positive, got {0}")]
    InvalidHorizon(usize),
    #[error("Threshold cannot be negative, got {0}")]
    NegativeThreshold(f64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_training_config_defaults() {
        let config = TrainingConfig::default();
        assert_eq!(config.learning_rate, 0.001);
        assert_eq!(config.batch_size, 32);
        assert_eq!(config.epochs, 100);
        assert_eq!(config.optimizer, OptimizerType::Adam);
        assert_eq!(config.validation_split, 0.2);
    }

    #[test]
    fn test_training_config_validation() {
        let mut config = TrainingConfig::default();

        // Valid config
        assert!(config.validate().is_ok());

        // Invalid learning rate
        config.learning_rate = 0.0;
        assert!(config.validate().is_err());

        // Reset and try batch size
        config.learning_rate = 0.001;
        config.batch_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_training_config_serializes() {
        let config = TrainingConfig::default();

        let json = serde_json::to_string(&config).unwrap();
        let parsed: TrainingConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.learning_rate, config.learning_rate);
        assert_eq!(parsed.batch_size, config.batch_size);
    }

    #[test]
    fn test_label_config_defaults() {
        let config = LabelConfig::default();
        assert_eq!(config.horizon, 1);
        assert_eq!(config.long_threshold, 0.005);
        assert_eq!(config.short_threshold, 0.005);
    }

    #[test]
    fn test_label_config_validation() {
        let mut config = LabelConfig::default();

        // Valid config
        assert!(config.validate().is_ok());

        // Invalid horizon
        config.horizon = 0;
        assert!(config.validate().is_err());

        // Invalid threshold
        config.horizon = 1;
        config.long_threshold = -0.1;
        assert!(config.validate().is_err());
    }
}
