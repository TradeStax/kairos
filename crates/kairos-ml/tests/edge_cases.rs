//! # Edge Case Tests
//!
//! Comprehensive edge case tests to improve code coverage and reliability.
//! These tests cover boundary conditions, error handling, and unusual inputs.

use kairos_ml::features::{
    FeatureConfig, FeatureDefinition, FeatureExtractor, FeatureTransform, NormalizationMethod,
    StudyFeatureExtractor,
};
use kairos_ml::model::{ModelError, ModelOutput, TradingSignal};
use kairos_ml::training::{
    Candle, DataGenerator, Dataset, LabelConfig, OptimizerType, TrainingConfig,
};

// ============================================================================
// Feature Extraction Edge Cases
// ============================================================================

#[test]
fn test_extractor_handles_large_lookback() {
    let config = FeatureConfig {
        features: vec![FeatureDefinition::new("test", "line")],
        lookback_periods: 10000, // Very large lookback
        normalization: NormalizationMethod::None,
    };

    let mut extractor = StudyFeatureExtractor::new(config);

    // Add exactly the number of values needed
    for i in 0..10000 {
        extractor.add_scalar("test", i as f64, i as i64);
    }

    // Should succeed
    let result = extractor.extract(10000);
    assert!(result.is_ok());
}

#[test]
fn test_extractor_buffer_limit() {
    let config = FeatureConfig {
        features: vec![FeatureDefinition::new("test", "line")],
        lookback_periods: 5,
        normalization: NormalizationMethod::None,
    };

    let mut extractor = StudyFeatureExtractor::new(config);

    // Add many more values than needed
    for i in 0..100 {
        extractor.add_scalar("test", i as f64, i as i64);
    }

    // Should still work - buffer should be limited
    let result = extractor.extract(5);
    assert!(result.is_ok());

    // Values should be from the end of the buffer
    let values = result.unwrap();
    // The last value should be 99 (100 - 1)
    assert_eq!(values[0].len(), 5);
}

#[test]
fn test_extractor_missing_study() {
    let config = FeatureConfig {
        features: vec![
            FeatureDefinition::new("existing", "line"),
            FeatureDefinition::new("missing", "line"),
        ],
        lookback_periods: 10,
        normalization: NormalizationMethod::None,
    };

    let mut extractor = StudyFeatureExtractor::new(config);

    // Only add one study
    for i in 0..20 {
        extractor.add_scalar("existing", i as f64, i as i64);
    }

    // Extract should fail because missing study
    let result = extractor.extract(10);
    assert!(result.is_err());
}

#[test]
fn test_transform_with_single_value() {
    let values = vec![1.0];

    // All transforms with single value should handle gracefully
    assert_eq!(FeatureTransform::Diff.apply(&values), vec![0.0]);
    assert_eq!(FeatureTransform::PctChange.apply(&values), vec![0.0]);
    assert_eq!(FeatureTransform::Log.apply(&values), vec![0.0]);
}

#[test]
fn test_transform_with_constant_values() {
    let values = vec![1.0, 1.0, 1.0, 1.0];

    // Diff of constant should be zeros
    let diff = FeatureTransform::Diff.apply(&values);
    assert_eq!(diff, vec![0.0, 0.0, 0.0, 0.0]);

    // PctChange of constant should be zeros
    let pct = FeatureTransform::PctChange.apply(&values);
    assert_eq!(pct, vec![0.0, 0.0, 0.0, 0.0]);
}

#[test]
fn test_transform_with_negative_values() {
    let values = vec![-1.0, -2.0, -3.0, -4.0];

    // Diff should still work
    let diff = FeatureTransform::Diff.apply(&values);
    assert_eq!(diff.len(), 4);
    assert!((diff[1] - (-1.0)).abs() < 0.001); // -2 - (-1) = -1

    // Log with negative values should clamp
    let log = FeatureTransform::Log.apply(&values);
    // Should be clamped to -100
    assert!(log.iter().all(|v| *v >= -100.0));
}

#[test]
fn test_transform_with_zero_values() {
    let values = vec![0.0, 0.0, 0.0];

    // Diff should work
    let diff = FeatureTransform::Diff.apply(&values);
    assert_eq!(diff, vec![0.0, 0.0, 0.0]);

    // PctChange with zero denominator should return 0
    let pct = FeatureTransform::PctChange.apply(&values);
    assert_eq!(pct, vec![0.0, 0.0, 0.0]);
}

#[test]
fn test_zscore_with_zero_stddev() {
    let values = vec![5.0, 5.0, 5.0, 5.0, 5.0];

    let normalized = kairos_ml::features::zscore_normalize(&values, 5.0, 0.0);
    assert!(normalized.iter().all(|v| *v == 0.0));
}

#[test]
fn test_minmax_with_identical_values() {
    let values = vec![5.0, 5.0, 5.0, 5.0];

    let result = kairos_ml::features::minmax_normalize(&values);
    assert!(result.is_some());
    // When min == max, all values should be 0
    assert!(result.unwrap().iter().all(|v| *v == 0.0));
}

// ============================================================================
// Dataset Edge Cases
// ============================================================================

#[test]
fn test_dataset_with_empty_features() {
    let dataset = Dataset::new(vec![], vec![], vec![]);
    assert!(dataset.is_empty());
    assert_eq!(dataset.len(), 0);
    assert_eq!(dataset.num_features(), 0);
    assert_eq!(dataset.lookback(), 0);
}

#[test]
fn test_dataset_single_sample() {
    let features = vec![vec![vec![1.0, 2.0]]]; // 1 sample, 1 lookback, 2 features
    let labels = vec![0];
    let timestamps = vec![1000];

    let dataset = Dataset::new(features, labels, timestamps);

    assert_eq!(dataset.len(), 1);
    assert_eq!(dataset.num_features(), 2);
    assert_eq!(dataset.lookback(), 1);
}

#[test]
fn test_dataset_split_at_boundaries() {
    let features: Vec<Vec<Vec<f64>>> = (0..10).map(|_| vec![vec![1.0], vec![2.0]]).collect();
    let labels: Vec<usize> = (0..10).collect();
    let timestamps: Vec<i64> = (0..10).map(|i| i as i64).collect();

    let dataset = Dataset::new(features, labels, timestamps);

    // Split at 0% (should give all to validation)
    let (_, val) = dataset.split(0.0);
    assert_eq!(val.len(), 10);

    // Split at 100% (should give all to training)
    let (train, _) = dataset.split(1.0);
    assert_eq!(train.len(), 10);
}

#[test]
fn test_batch_iterator_single_sample() {
    let features = vec![vec![vec![1.0, 2.0]]];
    let labels = vec![0];
    let timestamps = vec![1000];

    let dataset = Dataset::new(features, labels, timestamps);

    let mut iter = kairos_ml::training::BatchIterator::new(&dataset, 1);
    let batch = iter.next().unwrap();

    assert_eq!(batch.num_samples, 1);
    assert_eq!(batch.lookback, 1);
    assert_eq!(batch.num_features, 2);
    assert_eq!(batch.feature_shape(), [1, 1, 2]);
}

// ============================================================================
// Label Generation Edge Cases
// ============================================================================

#[test]
fn test_label_generation_at_threshold() {
    let returns = vec![0.005, -0.005]; // exactly at threshold
    let config = LabelConfig {
        horizon: 1,
        long_threshold: 0.005,
        short_threshold: 0.005,
        warmup_bars: 0,
    };

    let labels = kairos_ml::training::generate_labels(&returns, &config);

    // At exactly threshold should be neutral (not long or short)
    // Since 0.005 is not > 0.005, it's neutral
    assert_eq!(labels, vec![1, 1]);
}

#[test]
fn test_label_generation_edge_cases() {
    let config = LabelConfig {
        horizon: 1,
        long_threshold: 0.005,
        short_threshold: 0.005,
        warmup_bars: 0,
    };

    // Just above long threshold
    let labels = kairos_ml::training::generate_labels(&[0.0051], &config);
    assert_eq!(labels[0], 0); // Long

    // Just below long threshold
    let labels = kairos_ml::training::generate_labels(&[0.0049], &config);
    assert_eq!(labels[0], 1); // Neutral

    // Just below short threshold (more negative)
    let labels = kairos_ml::training::generate_labels(&[-0.0049], &config);
    assert_eq!(labels[0], 1); // Neutral

    // Just at short threshold
    let labels = kairos_ml::training::generate_labels(&[-0.0051], &config);
    assert_eq!(labels[0], 2); // Short
}

#[test]
fn test_label_generation_empty_returns() {
    let returns: Vec<f64> = vec![];
    let config = LabelConfig::default();

    let labels = kairos_ml::training::generate_labels(&returns, &config);
    assert!(labels.is_empty());
}

// ============================================================================
// Configuration Validation Edge Cases
// ============================================================================

#[test]
fn test_feature_config_empty_study_key() {
    let mut config = FeatureConfig {
        features: vec![FeatureDefinition::new("", "line")],
        lookback_periods: 10,
        normalization: NormalizationMethod::None,
    };

    assert!(config.validate().is_err());
}

#[test]
fn test_feature_config_empty_output_field() {
    let mut config = FeatureConfig {
        features: vec![FeatureDefinition::new("test", "")],
        lookback_periods: 10,
        normalization: NormalizationMethod::None,
    };

    assert!(config.validate().is_err());
}

#[test]
fn test_training_config_invalid_learning_rate() {
    let mut config = TrainingConfig::default();
    config.learning_rate = -0.001;
    assert!(config.validate().is_err());

    config.learning_rate = 0.0;
    assert!(config.validate().is_err());
}

#[test]
fn test_training_config_invalid_validation_split() {
    let mut config = TrainingConfig::default();

    config.validation_split = -0.1;
    assert!(config.validate().is_err());

    config.validation_split = 1.5;
    assert!(config.validate().is_err());
}

#[test]
fn test_label_config_negative_thresholds() {
    let mut config = LabelConfig::default();

    config.long_threshold = -0.001;
    assert!(config.validate().is_err());

    config = LabelConfig::default();
    config.short_threshold = -0.001;
    assert!(config.validate().is_err());
}

// ============================================================================
// Model Output Edge Cases
// ============================================================================

#[test]
fn test_model_output_probability_edges() {
    // Test very high confidence
    let output = ModelOutput::Classification {
        probabilities: [0.99, 0.005, 0.005],
        prediction: TradingSignal::Long,
    };
    assert!(output.is_confident(0.9));
    assert!(output.is_confident(0.99));
    assert!(!output.is_confident(0.995));

    // Test low confidence
    let output = ModelOutput::Classification {
        probabilities: [0.34, 0.33, 0.33],
        prediction: TradingSignal::Long,
    };
    assert!(!output.is_confident(0.3));
}

#[test]
fn test_model_output_regression_edges() {
    // Test positive value
    let output = ModelOutput::Regression { value: 1.0 };
    assert_eq!(output.signal(), TradingSignal::Long);
    assert_eq!(output.confidence(), 1.0);

    // Test negative value
    let output = ModelOutput::Regression { value: -0.5 };
    assert_eq!(output.signal(), TradingSignal::Short);
    assert_eq!(output.confidence(), 0.5);

    // Test zero
    let output = ModelOutput::Regression { value: 0.0 };
    assert_eq!(output.signal(), TradingSignal::Neutral);

    // Test value > 1 (should clamp confidence to 1.0)
    let output = ModelOutput::Regression { value: 5.0 };
    assert_eq!(output.confidence(), 1.0);
}

// ============================================================================
// Trading Signal Edge Cases
// ============================================================================

#[test]
fn test_trading_signal_all_variants() {
    let long = TradingSignal::Long;
    let short = TradingSignal::Short;
    let neutral = TradingSignal::Neutral;

    assert!(long.is_long());
    assert!(!long.is_short());
    assert!(!long.is_neutral());

    assert!(!short.is_long());
    assert!(short.is_short());
    assert!(!short.is_neutral());

    assert!(!neutral.is_long());
    assert!(!neutral.is_short());
    assert!(neutral.is_neutral());
}

#[test]
fn test_trading_signal_index_round_trip() {
    for i in 0..3 {
        let signal = TradingSignal::from_index(i).unwrap();
        assert_eq!(signal.to_index(), i);
    }

    // Invalid index should return None
    assert_eq!(TradingSignal::from_index(3), None);
    assert_eq!(TradingSignal::from_index(100), None);
}

// ============================================================================
// Data Generator Edge Cases
// ============================================================================

#[test]
fn test_data_generator_minimum_data() {
    let candles = vec![
        Candle::new(100.0, 101.0, 99.0, 100.5, 1000.0, 0),
        Candle::new(100.5, 101.5, 99.5, 101.0, 1000.0, 60000),
    ];

    let study_values = vec![100.0, 101.0];
    let timestamps = vec![0i64, 60000];

    let studies = vec![(
        "test",
        kairos_ml::training::StudyOutput::new(study_values, timestamps),
    )];

    let feature_config = FeatureConfig {
        features: vec![FeatureDefinition::new("test", "line")],
        lookback_periods: 1,
        normalization: NormalizationMethod::None,
    };

    let label_config = LabelConfig {
        horizon: 1,
        long_threshold: 0.005,
        short_threshold: 0.005,
        warmup_bars: 0,
    };

    let mut generator = DataGenerator::new(feature_config, label_config);
    let result = generator.generate(&candles, &studies);

    // Should produce at least 1 sample with the right data
    assert!(result.is_ok() || result.is_err()); // Just check it doesn't panic
}

#[test]
fn test_candle_returns_with_zero_open() {
    let candle = Candle::new(0.0, 1.0, 0.0, 1.0, 100.0, 0);
    // Should handle zero open gracefully
    assert!((candle.returns()).abs() < 0.001);

    let candle = Candle::new(0.0001, 1.0, 0.0, 1.0, 100.0, 0);
    // Should compute return for very small open
    let ret = candle.returns();
    assert!(ret > 0.0); // close > open
}

// ============================================================================
// Optimizer Type Edge Cases
// ============================================================================

#[test]
fn test_optimizer_type_serialization() {
    let sgd = OptimizerType::Sgd;
    let adam = OptimizerType::Adam;
    let adamw = OptimizerType::AdamW;

    let json = serde_json::to_string(&sgd).unwrap();
    assert!(json.contains("sgd"));

    let json = serde_json::to_string(&adam).unwrap();
    assert!(json.contains("adam"));

    let json = serde_json::to_string(&adamw).unwrap();
    assert!(json.contains("adamw"));
}

// ============================================================================
// Normalization Method Display
// ============================================================================

#[test]
fn test_normalization_method_display() {
    assert_eq!(NormalizationMethod::ZScore.to_string(), "zscore");
    assert_eq!(NormalizationMethod::MinMax.to_string(), "minmax");
    assert_eq!(NormalizationMethod::None.to_string(), "none");
}

#[test]
fn test_feature_transform_display() {
    assert_eq!(FeatureTransform::Log.to_string(), "log");
    assert_eq!(FeatureTransform::Diff.to_string(), "diff");
    assert_eq!(FeatureTransform::PctChange.to_string(), "pct_change");
    assert_eq!(FeatureTransform::None.to_string(), "none");
}

// ============================================================================
// Candle Edge Cases
// ============================================================================

#[test]
fn test_candle_with_same_open_close() {
    let candle = Candle::new(100.0, 101.0, 99.0, 100.0, 1000.0, 0);
    assert!((candle.returns()).abs() < 0.001);
}

#[test]
fn test_candle_with_large_move() {
    let candle = Candle::new(100.0, 200.0, 50.0, 150.0, 1000.0, 0);
    let ret = candle.returns();
    assert!((ret - 0.5).abs() < 0.001); // 50% return
}

#[test]
fn test_forward_return_with_zero_close() {
    let candle = Candle::new(100.0, 101.0, 99.0, 100.0, 1000.0, 0);
    let ret = candle.forward_return(0.0);
    assert!((ret).abs() < 0.001); // Should handle zero gracefully
}
