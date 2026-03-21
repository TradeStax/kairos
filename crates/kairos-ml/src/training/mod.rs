//! # Training Module
//!
//! This module provides training infrastructure for ML models.

pub mod config;
pub mod data_generator;
pub mod dataset;
pub mod training_loop;

pub use config::{LabelConfig, TrainingConfig};
pub use data_generator::{Candle, DataGenerator, DataGeneratorError, StudyOutput};
pub use dataset::Dataset;

use serde::{Deserialize, Serialize};

/// Optimizer type for training
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OptimizerType {
    /// Stochastic Gradient Descent
    Sgd,
    /// Adam optimizer
    #[default]
    Adam,
    /// AdamW optimizer (Adam with weight decay)
    AdamW,
}

/// Training metrics
#[derive(Debug, Clone, Default)]
pub struct TrainingMetrics {
    /// Epoch number
    pub epoch: usize,
    /// Training loss
    pub train_loss: f64,
    /// Validation loss
    pub val_loss: Option<f64>,
    /// Training accuracy
    pub train_accuracy: Option<f64>,
    /// Validation accuracy
    pub val_accuracy: Option<f64>,
}

/// Training result
#[derive(Debug)]
pub struct TrainingResult {
    /// Final training loss
    pub final_train_loss: f64,
    /// Final validation loss
    pub final_val_loss: Option<f64>,
    /// Number of epochs trained
    pub epochs_trained: usize,
    /// Whether early stopping was triggered
    pub early_stopped: bool,
    /// Metrics history
    pub metrics: Vec<TrainingMetrics>,
}

/// Generate labels from returns
pub fn generate_labels(returns: &[f64], config: &LabelConfig) -> Vec<usize> {
    let mut labels = Vec::with_capacity(returns.len());

    for r in returns {
        if *r > config.long_threshold {
            labels.push(0); // Long
        } else if *r < -config.short_threshold {
            labels.push(2); // Short
        } else {
            labels.push(1); // Neutral
        }
    }

    labels
}

/// Label index to trading signal
pub fn label_to_signal(label: usize) -> Option<super::model::TradingSignal> {
    use super::model::TradingSignal;
    match label {
        0 => Some(TradingSignal::Long),
        1 => Some(TradingSignal::Neutral),
        2 => Some(TradingSignal::Short),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_labels_long_threshold() {
        let returns = vec![0.01, -0.005, 0.02, 0.003, -0.01];
        let config = LabelConfig {
            horizon: 1,
            long_threshold: 0.005,
            short_threshold: 0.005,
            warmup_bars: 20,
        };

        let labels = generate_labels(&returns, &config);

        // 0.01 > 0.005 -> long (0)
        // -0.005 < -0.005 (equal, so neutral) -> neutral (1)
        // 0.02 > 0.005 -> long (0)
        // 0.003 < 0.005 -> neutral (1)
        // -0.01 < -0.005 -> short (2)
        assert_eq!(labels, vec![0, 1, 0, 1, 2]);
    }

    #[test]
    fn test_generate_labels_below_short_threshold() {
        let returns = vec![-0.02, -0.01, -0.005];
        let config = LabelConfig {
            horizon: 1,
            long_threshold: 0.005,
            short_threshold: 0.005,
            warmup_bars: 20,
        };

        let labels = generate_labels(&returns, &config);

        assert_eq!(labels, vec![2, 2, 1]); // short, short, neutral
    }

    #[test]
    fn test_label_to_signal() {
        use crate::model::TradingSignal;
        assert_eq!(label_to_signal(0), Some(TradingSignal::Long));
        assert_eq!(label_to_signal(1), Some(TradingSignal::Neutral));
        assert_eq!(label_to_signal(2), Some(TradingSignal::Short));
        assert_eq!(label_to_signal(3), None);
    }
}
