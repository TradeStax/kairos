//! # Kairos ML Strategy Module
//!
//! This crate provides PyTorch-based ML model support for Kairos, enabling:
//! - ML model inference during backtesting
//! - Model training on historical indicator data
//! - Seamless integration with existing strategy system
//!
//! ## Features
//!
//! - **Model Loading**: Load models from ONNX or PyTorch state dict format
//! - **Feature Extraction**: Convert any combination of studies to model input tensors
//! - **Inference Engine**: Batch prediction with signal output and confidence scores
//! - **Training Pipeline**: Train models using built-in indicators as features
//! - **ML Strategy**: `MlStrategy` wrapper implementing the `Strategy` trait
//!
//! ## Example
//!
//! ```ignore
//! use kairos_ml::{MlStrategy, MlStrategyConfig, FeatureConfig};
//!
//! let config = MlStrategyConfig::new(feature_config)
//!     .model_path("model.onnx");
//!
//! let strategy = MlStrategy::new(config);
//! ```
//!
//! ## Architecture
//!
//! The crate is organized into the following modules:
//!
//! - `model`: Model loading, inference, and registry
//! - `features`: Study-to-feature extraction pipeline
//! - `training`: Model training infrastructure
//! - `strategy`: `MlStrategy` implementing the `Strategy` trait

// Re-export commonly used types
pub use features::{FeatureConfig, FeatureDefinition, FeatureExtractor};
pub use model::{Model, ModelOutput, TradingSignal};
pub use strategy::{MlStrategy, MlStrategyConfig};
pub use training::{Candle, DataGenerator, Dataset, LabelConfig, StudyOutput, TrainingConfig};

// Re-export training loop items
pub use training::training_loop::{LoggingCallback, TrainingCallback};

// Module declarations
pub mod features;
pub mod model;
pub mod strategy;
pub mod training;

// Re-export tch when available
#[cfg(feature = "tch")]
pub use tch;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_modules_exist() {
        // Verify all expected modules are accessible
        let _ = model::ModelOutput::Regression { value: 0.0 };
        let _ = features::FeatureConfig {
            features: vec![],
            lookback_periods: 20,
            normalization: features::NormalizationMethod::ZScore,
        };
        let _ = training::TrainingConfig::default();
    }

    #[test]
    fn test_trading_signal_variants() {
        use model::TradingSignal::*;

        assert!(matches!(TradingSignal::Long, Long));
        assert!(matches!(TradingSignal::Short, Short));
        assert!(matches!(TradingSignal::Neutral, Neutral));
    }
}

/// Example compilation tests
#[cfg(test)]
mod example_tests {
    //! Tests to verify example code compiles successfully.
    //!
    //! These tests are ignored by default and can be run with:
    //! `cargo test --test integration_examples -- --include-ignored`

    /// Test that the training example compiles by checking example imports are valid.
    /// This verifies the public API used by the example is stable.
    #[test]
    #[ignore = "example only - verify examples compile with: cargo check --examples"]
    fn test_train_example_api_usage() {
        use kairos_ml::model::tch_impl::TchModel;
        use kairos_ml::training::training_loop::train;
        use kairos_ml::training::{Dataset, LabelConfig, OptimizerType, TrainingConfig};

        // Verify the types and functions used in the example exist
        let label_config = LabelConfig {
            horizon: 5,
            long_threshold: 0.005,
            short_threshold: 0.005,
            warmup_bars: 20,
        };

        let config = TrainingConfig {
            model_type: kairos_ml::training::config::ModelType::Mlp,
            learning_rate: 0.001,
            batch_size: 32,
            epochs: 50,
            optimizer: OptimizerType::Adam,
            weight_decay: 0.0001,
            label_config,
            validation_split: 0.2,
            early_stopping_patience: 10,
        };

        // Verify TrainingConfig implements required traits
        fn _assert_default<T: Default>() {}
        _assert_default::<TrainingConfig>();

        // Verify TchModel can be created with expected parameters
        let model = TchModel::new(60, 64, 3, "simple_model");
        assert_eq!(model.name(), "simple_model");
    }

    /// Test that the backtest example API usage is valid.
    #[test]
    #[ignore = "example only - verify examples compile with: cargo check --examples"]
    fn test_backtest_example_api_usage() {
        use kairos_ml::features::{FeatureConfig, FeatureDefinition, NormalizationMethod};
        use kairos_ml::model::{ModelOutput, TradingSignal};
        use kairos_ml::strategy::{MlStrategy, MlStrategyConfig};

        // Verify FeatureConfig can be created
        let feature_config = FeatureConfig {
            features: vec![
                FeatureDefinition {
                    study_key: "sma_20".to_string(),
                    output_field: "line".to_string(),
                    transform: None,
                },
                FeatureDefinition {
                    study_key: "rsi_14".to_string(),
                    output_field: "line".to_string(),
                    transform: None,
                },
            ],
            lookback_periods: 20,
            normalization: NormalizationMethod::ZScore,
        };

        // Verify MlStrategyConfig can be created
        let strategy_config = MlStrategyConfig {
            model_path: "trained_model.pt".to_string(),
            feature_config,
            signal_threshold_long: 0.6,
            signal_threshold_short: 0.6,
            use_confidence_for_sizing: true,
        };

        // Verify TradingSignal variants
        let signals = vec![
            TradingSignal::Long,
            TradingSignal::Short,
            TradingSignal::Neutral,
        ];
        assert_eq!(signals.len(), 3);
    }
}
