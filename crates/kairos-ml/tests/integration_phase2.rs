//! # Phase 2 Integration Tests
//!
//! Integration tests for Phase 2: Feature Extraction Pipeline

use kairos_ml::features::extractor::StudyFeatureExtractor;
use kairos_ml::features::{
    FeatureConfig, FeatureDefinition, FeatureExtractor, NormalizationMethod,
};
use kairos_ml::training::{Candle, DataGenerator, StudyOutput};

/// Integration test: extract features from multiple studies
#[test]
fn test_extract_multiple_studies_as_features() {
    // Create feature config with SMA and RSI
    let config = FeatureConfig {
        features: vec![
            FeatureDefinition::new("sma_20", "line"),
            FeatureDefinition::new("rsi_14", "value"),
        ],
        lookback_periods: 20,
        normalization: NormalizationMethod::None,
    };

    let mut extractor = StudyFeatureExtractor::new(config.clone());

    // Generate test data: 50 bars
    for i in 0..50 {
        let timestamp = i as i64 * 60000;

        // Simulate SMA values
        let sma_value = 100.0 + (i as f64 * 0.5);
        extractor.add_scalar("sma_20", sma_value, timestamp);

        // Simulate RSI values
        let rsi_value = 50.0 + ((i % 20) as f64 - 10.0) * 2.0;
        extractor.add_scalar("rsi_14", rsi_value, timestamp);
    }

    // Extract features with lookback of 20
    let result = extractor.extract(20);
    assert!(result.is_ok());

    let features = result.unwrap();

    // Should have 2 features (SMA and RSI)
    assert_eq!(features.len(), 2, "Should have 2 features");

    // Each feature should have 20 values (lookback)
    assert_eq!(features[0].len(), 20, "SMA should have 20 values");
    assert_eq!(features[1].len(), 20, "RSI should have 20 values");
}

/// Integration test: feature extraction with normalization
#[test]
fn test_feature_extraction_with_normalization() {
    let config = FeatureConfig {
        features: vec![FeatureDefinition::new("indicator", "value")],
        lookback_periods: 10,
        normalization: NormalizationMethod::ZScore,
    };

    let mut extractor = StudyFeatureExtractor::new(config);

    // Add values with known mean and std
    for i in 1..=10 {
        extractor.add_scalar("indicator", i as f64, i as i64);
    }

    let result = extractor.extract(10).unwrap();

    // After z-score normalization, mean should be ~0
    let mean: f64 = result[0].iter().sum::<f64>() / result[0].len() as f64;
    assert!((mean - 0.0).abs() < 0.001, "Normalized mean should be ~0");
}

/// Integration test: feature extraction with transform
#[test]
fn test_feature_extraction_with_transform() {
    let config = FeatureConfig {
        features: vec![FeatureDefinition {
            study_key: "price".to_string(),
            output_field: "line".to_string(),
            transform: Some(kairos_ml::features::FeatureTransform::Diff),
            name: None,
        }],
        lookback_periods: 5,
        normalization: NormalizationMethod::None,
    };

    let mut extractor = StudyFeatureExtractor::new(config);

    // Add price values
    let prices = vec![100.0, 101.0, 103.0, 102.0, 105.0];
    for (i, price) in prices.iter().enumerate() {
        extractor.add_scalar("price", *price, i as i64);
    }

    let result = extractor.extract(5).unwrap();

    // First value should be 0 (no previous value for diff)
    assert!((result[0][0] - 0.0).abs() < 0.001);

    // Subsequent values should be differences
    assert!((result[0][1] - 1.0).abs() < 0.001); // 101 - 100
    assert!((result[0][2] - 2.0).abs() < 0.001); // 103 - 101
    assert!((result[0][3] - (-1.0)).abs() < 0.001); // 102 - 103
    assert!((result[0][4] - 3.0).abs() < 0.001); // 105 - 102
}

/// Integration test: data generator with real-like data
#[test]
fn test_data_generator_realistic_data() {
    // Create realistic candle data
    let mut candles = Vec::new();
    let base_price = 100.0;

    for i in 0..100 {
        let open = base_price + (i as f64 * 0.1).sin() * 5.0;
        let close = open + (i as f64 % 5.0 - 2.5) * 0.5;
        candles.push(Candle::new(
            open,
            open.max(close) + 0.3,
            open.min(close) - 0.2,
            close,
            1000.0 + i as f64 * 10.0,
            i as i64 * 60000,
        ));
    }

    // Create study outputs
    let sma_values: Vec<f64> = (0..100).map(|i| base_price + (i as f64 * 0.1)).collect();
    let timestamps: Vec<i64> = (0..100).map(|i| i as i64 * 60000).collect();

    let rsi_values: Vec<f64> = (0..100)
        .map(|i| 50.0 + ((i % 14) as f64 - 7.0) * 3.0)
        .collect();

    let studies = vec![
        ("sma", StudyOutput::new(sma_values, timestamps.clone())),
        ("rsi", StudyOutput::new(rsi_values, timestamps.clone())),
    ];

    // Create feature config
    let feature_config = FeatureConfig {
        features: vec![
            FeatureDefinition::new("sma", "line"),
            FeatureDefinition::new("rsi", "value"),
        ],
        lookback_periods: 10,
        normalization: NormalizationMethod::None,
    };

    let label_config = kairos_ml::training::LabelConfig {
        horizon: 5,
        long_threshold: 0.01,
        short_threshold: 0.01,
        warmup_bars: 20,
    };

    let mut generator = DataGenerator::new(feature_config, label_config);

    let dataset = generator.generate(&candles, &studies);
    assert!(dataset.is_ok(), "Dataset generation should succeed");

    let dataset = dataset.unwrap();

    // Verify dataset properties
    assert!(dataset.len() > 0, "Dataset should have samples");
    assert_eq!(dataset.num_features(), 2, "Should have 2 features");
    assert_eq!(dataset.lookback(), 10, "Lookback should be 10");

    // Verify features have correct shape
    for sample in &dataset.features {
        assert_eq!(sample.len(), 10, "Each sample should have 10 timesteps");
        assert_eq!(sample[0].len(), 2, "Each timestep should have 2 features");
    }
}

/// Integration test: data generator with train/validation split
#[test]
fn test_data_generator_with_split() {
    // Create small dataset
    let candles: Vec<Candle> = (0..50)
        .map(|i| {
            Candle::new(
                100.0 + i as f64,
                101.0 + i as f64,
                99.0 + i as f64,
                100.5 + i as f64,
                1000.0,
                i as i64 * 60000,
            )
        })
        .collect();

    let study_values: Vec<f64> = (0..50).map(|i| 100.0 + i as f64 * 0.5).collect();
    let timestamps: Vec<i64> = (0..50).map(|i| i as i64 * 60000).collect();

    let studies = vec![("study", StudyOutput::new(study_values, timestamps))];

    let feature_config = FeatureConfig {
        features: vec![FeatureDefinition::new("study", "line")],
        lookback_periods: 5,
        normalization: NormalizationMethod::None,
    };

    let label_config = kairos_ml::training::LabelConfig {
        horizon: 1,
        long_threshold: 0.005,
        short_threshold: 0.005,
        warmup_bars: 5,
    };

    let mut generator = DataGenerator::new(feature_config, label_config);
    let dataset = generator.generate(&candles, &studies).unwrap();

    // Split dataset
    let (train, val) = dataset.split(0.2);

    // Verify split sizes
    let total = train.len() + val.len();
    assert_eq!(total, dataset.len(), "All samples should be preserved");
    assert!(train.len() > val.len(), "Training set should be larger");
}

/// Integration test: insufficient data handling
#[test]
fn test_insufficient_data_error() {
    // Create very small dataset
    let candles: Vec<Candle> = (0..5)
        .map(|i| Candle::new(100.0, 101.0, 99.0, 100.5, 1000.0, i as i64))
        .collect();

    let study_values: Vec<f64> = (0..5).map(|i| 100.0 + i as f64).collect();
    let timestamps: Vec<i64> = (0..5).map(|i| i as i64).collect();

    let studies = vec![("study", StudyOutput::new(study_values, timestamps))];

    let feature_config = FeatureConfig {
        features: vec![FeatureDefinition::new("study", "line")],
        lookback_periods: 20, // Requires more than we have
        normalization: NormalizationMethod::None,
    };

    let label_config = kairos_ml::training::LabelConfig {
        horizon: 1,
        long_threshold: 0.005,
        short_threshold: 0.005,
        warmup_bars: 2,
    };

    let mut generator = DataGenerator::new(feature_config, label_config);
    let result = generator.generate(&candles, &studies);

    assert!(result.is_err(), "Should fail with insufficient data");
}

/// Integration test: multiple feature transforms
#[test]
fn test_multiple_feature_transforms() {
    let config = FeatureConfig {
        features: vec![FeatureDefinition {
            study_key: "price".to_string(),
            output_field: "line".to_string(),
            transform: Some(kairos_ml::features::FeatureTransform::PctChange),
            name: None,
        }],
        lookback_periods: 10,
        normalization: NormalizationMethod::None,
    };

    let mut extractor = StudyFeatureExtractor::new(config);

    // Add values: 100, 101, 102, 103, 104, 105, 106, 107, 108, 109
    for i in 0..10 {
        extractor.add_scalar("price", 100.0 + i as f64, i as i64);
    }

    let result = extractor.extract(10).unwrap();

    // First value should be 0 (no previous for pct_change)
    assert!((result[0][0] - 0.0).abs() < 0.001);

    // Subsequent values should be ~0.01 (1% change)
    for i in 1..10 {
        assert!((result[0][i] - 0.01).abs() < 0.001);
    }
}

/// Integration test: data generator with different label thresholds
#[test]
fn test_data_generator_label_thresholds() {
    let candles: Vec<Candle> = (0..100)
        .map(|i| {
            let trend = (i as f64 / 10.0).sin() * 5.0;
            let close = 100.0 + i as f64 * 0.1 + trend;
            Candle::new(
                close - 0.5,
                close + 0.5,
                close - 1.0,
                close,
                1000.0,
                i as i64 * 60000,
            )
        })
        .collect();

    let study_values: Vec<f64> = (0..100).map(|i| 100.0 + i as f64 * 0.1).collect();
    let timestamps: Vec<i64> = (0..100).map(|i| i as i64 * 60000).collect();

    let studies = vec![("study", StudyOutput::new(study_values, timestamps))];

    let feature_config = FeatureConfig {
        features: vec![FeatureDefinition::new("study", "line")],
        lookback_periods: 5,
        normalization: NormalizationMethod::None,
    };

    // Test with strict thresholds (only large moves)
    let strict_label_config = kairos_ml::training::LabelConfig {
        horizon: 1,
        long_threshold: 0.02, // 2%
        short_threshold: 0.02,
        warmup_bars: 10,
    };

    let mut generator = DataGenerator::new(feature_config.clone(), strict_label_config);
    let strict_dataset = generator.generate(&candles, &studies).unwrap();

    // Test with loose thresholds
    let loose_label_config = kairos_ml::training::LabelConfig {
        horizon: 1,
        long_threshold: 0.001, // 0.1%
        short_threshold: 0.001,
        warmup_bars: 10,
    };

    let mut generator = DataGenerator::new(feature_config, loose_label_config);
    let loose_dataset = generator.generate(&candles, &studies).unwrap();

    // Loose thresholds should produce more extreme labels (0 or 2)
    let strict_extremes = strict_dataset.labels.iter().filter(|&&l| l != 1).count();
    let loose_extremes = loose_dataset.labels.iter().filter(|&&l| l != 1).count();

    assert!(
        loose_extremes >= strict_extremes,
        "Loose thresholds should produce more extreme labels"
    );
}
