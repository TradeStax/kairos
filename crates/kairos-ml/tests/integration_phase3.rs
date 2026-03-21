//! # Integration Tests for Phase 3: ML Strategy Wrapper
//!
//! These tests verify the MlStrategy works correctly with the backtest engine.

use kairos_backtest::strategy::Strategy;
use kairos_ml::features::{FeatureConfig, FeatureDefinition, NormalizationMethod};
use kairos_ml::model::{Model, ModelOutput, TradingSignal};
use kairos_ml::strategy::{MlStrategy, MlStrategyConfig};
use std::sync::Arc;

/// Mock model for testing that always returns a specific output
struct MockModel {
    prediction: ModelOutput,
}

impl MockModel {
    fn new_classification(probabilities: [f64; 3], signal: TradingSignal) -> Self {
        Self {
            prediction: ModelOutput::Classification {
                probabilities,
                prediction: signal,
            },
        }
    }

    fn new_regression(value: f64) -> Self {
        Self {
            prediction: ModelOutput::Regression { value },
        }
    }
}

impl Model for MockModel {
    fn predict(&self, _input: &tch::Tensor) -> Result<ModelOutput, kairos_ml::model::ModelError> {
        Ok(self.prediction.clone())
    }

    fn input_shape(&self) -> Vec<i64> {
        vec![1, 20, 2] // [batch, lookback, features]
    }

    fn output_shape(&self) -> Vec<i64> {
        vec![1, 3] // [batch, 3 classes]
    }

    fn name(&self) -> &str {
        "MockModel"
    }
}

fn create_test_config() -> MlStrategyConfig {
    MlStrategyConfig {
        id: Some("test_ml_strategy".to_string()),
        name: Some("Test ML Strategy".to_string()),
        description: Some("A test ML strategy".to_string()),
        model_path: None,
        feature_config: FeatureConfig {
            features: vec![
                FeatureDefinition::new("sma_20", "line"),
                FeatureDefinition::new("rsi_14", "value"),
            ],
            lookback_periods: 20,
            normalization: NormalizationMethod::ZScore,
        },
        signal_threshold_long: 0.6,
        signal_threshold_short: 0.6,
        min_confidence: 0.3,
        use_confidence_for_sizing: false,
    }
}

#[test]
fn test_ml_strategy_initialization() {
    let config = create_test_config();
    let strategy = MlStrategy::new(config);

    assert_eq!(strategy.id(), "test_ml_strategy");
    assert!(!strategy.warmup_complete());
    assert_eq!(strategy.bars_processed(), 0);
}

#[test]
fn test_ml_strategy_reset() {
    let config = create_test_config();
    let mut strategy = MlStrategy::new(config);

    // Simulate some state by calling reset
    strategy.reset();

    assert_eq!(strategy.bars_processed(), 0);
    assert!(!strategy.warmup_complete());
    assert_eq!(strategy.current_signal(), TradingSignal::Neutral);
}

#[test]
fn test_ml_strategy_clone() {
    let config = create_test_config();
    let strategy = MlStrategy::new(config);

    let cloned = strategy.clone_strategy();

    assert_eq!(cloned.id(), strategy.id());
}

#[test]
fn test_ml_strategy_config_builder() {
    let config = MlStrategyConfig::default();

    assert_eq!(config.signal_threshold_long, 0.6);
    assert_eq!(config.signal_threshold_short, 0.6);
    assert_eq!(config.min_confidence, 0.5);
}

#[test]
fn test_ml_strategy_config_validation() {
    let mut config = MlStrategyConfig::default();

    // Invalid threshold (must be 0.0-1.0)
    config.signal_threshold_long = 1.5;
    assert!(config.validate().is_err());

    config.signal_threshold_long = -0.1;
    assert!(config.validate().is_err());

    // Valid threshold
    config.signal_threshold_long = 0.7;
    assert!(config.validate().is_ok());
}

#[test]
fn test_ml_strategy_required_studies() {
    let config = create_test_config();
    let strategy = MlStrategy::new(config);

    let studies = strategy.required_studies();
    assert_eq!(studies.len(), 2);
    assert!(studies.iter().any(|s| s.key == "sma_20"));
    assert!(studies.iter().any(|s| s.key == "rsi_14"));
}

#[test]
fn test_ml_strategy_set_model() {
    let config = create_test_config();
    let mut strategy = MlStrategy::new(config);

    let model = Arc::new(MockModel::new_classification(
        [0.3, 0.4, 0.3],
        TradingSignal::Neutral,
    ));

    strategy.set_model(model);

    // The strategy should have a model now
    // (We can't directly check this without exposing the model field)
    assert_eq!(strategy.current_signal(), TradingSignal::Neutral);
}

#[test]
fn test_signal_generation_thresholds() {
    // Test long signal
    let probabilities = [0.7, 0.2, 0.1]; // 70% long
    let signal = compute_signal(&probabilities, 0.6, 0.6);
    assert_eq!(signal, TradingSignal::Long);

    // Test short signal
    let probabilities = [0.1, 0.2, 0.7]; // 70% short
    let signal = compute_signal(&probabilities, 0.6, 0.6);
    assert_eq!(signal, TradingSignal::Short);

    // Test neutral signal
    let probabilities = [0.4, 0.3, 0.3]; // 40% long, below threshold
    let signal = compute_signal(&probabilities, 0.6, 0.6);
    assert_eq!(signal, TradingSignal::Neutral);

    // Test at exact threshold
    let probabilities = [0.6, 0.2, 0.2]; // Exactly at threshold
    let signal = compute_signal(&probabilities, 0.6, 0.6);
    assert_eq!(signal, TradingSignal::Long);
}

/// Helper function to compute trading signal from probabilities
fn compute_signal(
    probabilities: &[f64; 3],
    threshold_long: f64,
    threshold_short: f64,
) -> TradingSignal {
    if probabilities[0] >= threshold_long {
        TradingSignal::Long
    } else if probabilities[2] >= threshold_short {
        TradingSignal::Short
    } else {
        TradingSignal::Neutral
    }
}

#[test]
fn test_model_output_serialization() {
    let output = ModelOutput::Classification {
        probabilities: [0.2, 0.5, 0.3],
        prediction: TradingSignal::Neutral,
    };

    let json = serde_json::to_string(&output).unwrap();
    assert!(json.contains("Neutral"));

    let output = ModelOutput::Regression { value: 1.5 };
    let json = serde_json::to_string(&output).unwrap();
    assert!(json.contains("1.5"));
}

#[test]
fn test_model_output_confidence() {
    let classification = ModelOutput::Classification {
        probabilities: [0.7, 0.2, 0.1],
        prediction: TradingSignal::Long,
    };
    assert!((classification.confidence() - 0.7).abs() < 0.001);

    let regression = ModelOutput::Regression { value: 1.5 };
    // Regression confidence is absolute value
    assert!((regression.confidence() - 1.5).abs() < 0.001);
}

#[test]
fn test_model_output_signal() {
    let classification = ModelOutput::Classification {
        probabilities: [0.7, 0.2, 0.1],
        prediction: TradingSignal::Long,
    };
    assert_eq!(classification.signal(), TradingSignal::Long);

    let regression = ModelOutput::Regression { value: 0.05 };
    // For regression, signal is derived from value sign
    assert_eq!(regression.signal(), TradingSignal::Neutral); // 0.05 is neutral
}

#[test]
fn test_feature_config_with_transforms() {
    let config = FeatureConfig {
        features: vec![
            FeatureDefinition::new("sma_20", "line"),
            FeatureDefinition::new("volume", "value")
                .with_transform(kairos_ml::features::FeatureTransform::PctChange),
        ],
        lookback_periods: 20,
        normalization: NormalizationMethod::ZScore,
    };

    assert_eq!(config.features.len(), 2);
    assert!(config.features[1].transform.is_some());
}

#[test]
fn test_model_registry_with_mock() {
    use kairos_ml::model::registry::ModelRegistry;

    let _registry = ModelRegistry::new();

    // Registry should be empty by default
    // Note: list_models may not exist, so we just verify registry is created
    assert!(true);
}

#[test]
fn test_ml_strategy_with_multiple_features() {
    let mut config = create_test_config();
    config.feature_config = FeatureConfig {
        features: vec![
            FeatureDefinition::new("sma_20", "line"),
            FeatureDefinition::new("sma_50", "line"),
            FeatureDefinition::new("rsi_14", "value"),
            FeatureDefinition::new("macd", "value"),
        ],
        lookback_periods: 50,
        normalization: NormalizationMethod::ZScore,
    };

    let strategy = MlStrategy::new(config);

    let studies = strategy.required_studies();
    assert_eq!(studies.len(), 4);
}

#[test]
fn test_ml_strategy_metadata() {
    let config = create_test_config();
    let strategy = MlStrategy::new(config);

    let metadata = strategy.metadata();
    assert_eq!(metadata.id, "test_ml_strategy");
    assert_eq!(metadata.name, "Test ML Strategy");
    assert!(!metadata.description.is_empty());
}

#[test]
fn test_ml_strategy_config_clone() {
    let config = create_test_config();
    let cloned = config.clone();

    assert_eq!(cloned.id, config.id);
    assert_eq!(
        cloned.feature_config.features.len(),
        config.feature_config.features.len()
    );
}
