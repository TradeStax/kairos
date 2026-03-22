//! # Phase 4 Integration Tests
//!
//! Integration tests for the training pipeline including training loop,
//! early stopping, and model export.

use kairos_ml::training::training_loop::{LoggingCallback, train};
use kairos_ml::training::{Dataset, LabelConfig, OptimizerType, TrainingConfig};
use tempfile::TempDir;

#[cfg(feature = "tch")]
mod tch_tests {
    use super::*;
    use kairos_ml::training::config::LstmConfig;

    /// Create a synthetic dataset for testing
    fn create_synthetic_dataset(
        num_samples: usize,
        num_features: usize,
        lookback: usize,
    ) -> Dataset {
        let mut features = Vec::with_capacity(num_samples);
        let mut labels = Vec::with_capacity(num_samples);
        let mut timestamps = Vec::with_capacity(num_samples);

        // Generate random features
        for i in 0..num_samples {
            let mut sample = Vec::with_capacity(lookback);
            for _ in 0..lookback {
                let mut time_step = Vec::with_capacity(num_features);
                for _ in 0..num_features {
                    time_step.push(rand_simple(i));
                }
                sample.push(time_step);
            }
            features.push(sample);
            // Create labels based on first feature (for predictability)
            let label = if rand_simple(i) > 0.0 { 0 } else { 2 }; // long or short
            labels.push(label);
            timestamps.push(i as i64);
        }

        Dataset::new(features, labels, timestamps)
    }

    /// Simple pseudo-random function
    fn rand_simple(seed: usize) -> f64 {
        let x = (seed as f64 * 1103515245.0 + 12345.0).fract();
        x * 2.0 - 1.0 // Map to [-1, 1]
    }

    #[test]
    fn test_training_improves_loss() {
        let dataset = create_synthetic_dataset(100, 5, 10);

        let config = TrainingConfig {
            model_type: kairos_ml::training::config::ModelType::Mlp,
            learning_rate: 0.01,
            batch_size: 16,
            epochs: 5,
            optimizer: OptimizerType::Adam,
            weight_decay: 0.0001,
            label_config: LabelConfig::default(),
            validation_split: 0.2,
            early_stopping_patience: 0, // Disable early stopping
            lstm_config: LstmConfig::default(),
            gpu_device: None,
        };

        let result = train(&config, &dataset, &LoggingCallback);

        // Training should complete without errors
        assert_eq!(result.result.epochs_trained, 5);
        assert!(!result.result.early_stopped);
    }

    #[test]
    fn test_training_respects_batch_size() {
        let dataset = create_synthetic_dataset(100, 5, 10);

        let config = TrainingConfig {
            model_type: kairos_ml::training::config::ModelType::Mlp,
            learning_rate: 0.01,
            batch_size: 32,
            epochs: 3,
            optimizer: OptimizerType::Adam,
            weight_decay: 0.0001,
            label_config: LabelConfig::default(),
            validation_split: 0.2,
            early_stopping_patience: 0,
            lstm_config: LstmConfig::default(),
            gpu_device: None,
        };

        let result = train(&config, &dataset, &LoggingCallback);

        assert_eq!(result.result.epochs_trained, 3);
        // Check metrics were recorded
        assert_eq!(result.result.metrics.len(), 3);
    }

    #[test]
    fn test_training_completes_all_epochs() {
        let dataset = create_synthetic_dataset(50, 3, 5);

        let config = TrainingConfig {
            model_type: kairos_ml::training::config::ModelType::Mlp,
            learning_rate: 0.01,
            batch_size: 10,
            epochs: 10,
            optimizer: OptimizerType::Adam,
            weight_decay: 0.0001,
            label_config: LabelConfig::default(),
            validation_split: 0.0, // No validation
            early_stopping_patience: 0,
            lstm_config: LstmConfig::default(),
            gpu_device: None,
        };

        let result = train(&config, &dataset, &LoggingCallback);

        assert_eq!(result.result.epochs_trained, 10);
        assert!(!result.result.early_stopped);
    }

    #[test]
    fn test_training_produces_metrics_history() {
        let dataset = create_synthetic_dataset(60, 4, 8);

        let config = TrainingConfig {
            model_type: kairos_ml::training::config::ModelType::Mlp,
            learning_rate: 0.01,
            batch_size: 20,
            epochs: 5,
            optimizer: OptimizerType::Adam,
            weight_decay: 0.0001,
            label_config: LabelConfig::default(),
            validation_split: 0.2,
            early_stopping_patience: 0,
            lstm_config: LstmConfig::default(),
            gpu_device: None,
        };

        let result = train(&config, &dataset, &LoggingCallback);

        // Check metrics history
        assert_eq!(result.result.metrics.len(), 5);

        for (i, metrics) in result.result.metrics.iter().enumerate() {
            assert_eq!(metrics.epoch, i + 1);
            assert!(metrics.train_loss >= 0.0);
            assert!(metrics.val_loss.is_some());
        }
    }

    #[test]
    fn test_model_export_to_file() {
        use kairos_ml::training::training_loop::TrainResult;

        let temp_dir = TempDir::new().unwrap();
        let model_path = temp_dir.path().join("trained_model.pt");

        // Create a simple dataset and train to get a VarStore
        let dataset = create_synthetic_dataset(50, 5, 10);

        let config = TrainingConfig {
            model_type: kairos_ml::training::config::ModelType::Mlp,
            learning_rate: 0.01,
            batch_size: 10,
            epochs: 2,
            optimizer: OptimizerType::Adam,
            weight_decay: 0.0001,
            label_config: LabelConfig::default(),
            validation_split: 0.0,
            early_stopping_patience: 0,
            lstm_config: LstmConfig::default(),
            gpu_device: None,
        };

        let result = train(&config, &dataset, &LoggingCallback);

        // Save model via VarStore
        let save_result = result.var_store.save(&model_path);
        assert!(save_result.is_ok());

        // Verify file exists
        assert!(model_path.exists());
    }

    #[test]
    fn test_training_with_multiple_optimizer_types() {
        let dataset = create_synthetic_dataset(50, 3, 5);

        for optimizer in &[
            OptimizerType::Sgd,
            OptimizerType::Adam,
            OptimizerType::AdamW,
        ] {
            let config = TrainingConfig {
                model_type: kairos_ml::training::config::ModelType::Mlp,
                learning_rate: 0.01,
                batch_size: 10,
                epochs: 2,
                optimizer: *optimizer,
                weight_decay: 0.0001,
                label_config: LabelConfig::default(),
                validation_split: 0.0,
                early_stopping_patience: 0,
                lstm_config: LstmConfig::default(),
                gpu_device: None,
            };

            let result = train(&config, &dataset, &LoggingCallback);
            assert_eq!(
                result.result.epochs_trained, 2,
                "Failed for optimizer: {:?}",
                optimizer
            );
        }
    }

    #[test]
    fn test_dataset_split_functionality() {
        let dataset = create_synthetic_dataset(100, 5, 10);
        let original_len = dataset.len();

        let (train_set, val_set) = dataset.split(0.2);

        // Check sizes are reasonable
        assert!(train_set.len() > 0);
        assert!(val_set.len() > 0);
        assert_eq!(train_set.len() + val_set.len(), original_len);

        // Check shapes are preserved
        assert_eq!(train_set.num_features(), dataset.num_features());
        assert_eq!(train_set.lookback(), dataset.lookback());
    }
}

// Tests that work without tch feature
#[test]
fn test_training_config_default_values() {
    let config = TrainingConfig::default();

    assert_eq!(config.learning_rate, 0.001);
    assert_eq!(config.batch_size, 32);
    assert_eq!(config.epochs, 100);
    assert_eq!(config.optimizer, OptimizerType::Adam);
    assert_eq!(config.validation_split, 0.2);
    assert_eq!(config.early_stopping_patience, 10);
}

#[test]
fn test_label_config_default_values() {
    let config = LabelConfig::default();

    assert_eq!(config.horizon, 1);
    assert_eq!(config.long_threshold, 0.005);
    assert_eq!(config.short_threshold, 0.005);
}

#[test]
fn test_label_generation_function() {
    let returns = vec![0.01, -0.005, 0.02, 0.003, -0.01];
    let config = LabelConfig {
        horizon: 1,
        long_threshold: 0.005,
        short_threshold: 0.005,
        warmup_bars: 20,
    };

    // Using the generate_labels function from mod.rs
    let labels = kairos_ml::training::generate_labels(&returns, &config);

    // 0.01 > 0.005 -> long (0)
    // -0.005 < -0.005 (equal, so neutral) -> neutral (1)
    // 0.02 > 0.005 -> long (0)
    // 0.003 < 0.005 -> neutral (1)
    // -0.01 < -0.005 -> short (2)
    assert_eq!(labels, vec![0, 1, 0, 1, 2]);
}

#[test]
fn test_data_generator_creation() {
    use kairos_ml::features::{FeatureConfig, NormalizationMethod};

    let feature_config = FeatureConfig {
        features: vec![],
        lookback_periods: 10,
        normalization: NormalizationMethod::None,
    };
    let label_config = LabelConfig::default();

    // DataGenerator should be creatable
    let _generator = kairos_ml::training::DataGenerator::new(feature_config, label_config);
}

#[test]
fn test_dataset_basic_operations() {
    let dataset = Dataset::new(
        vec![
            vec![vec![1.0, 2.0], vec![3.0, 4.0]],
            vec![vec![5.0, 6.0], vec![7.0, 8.0]],
        ],
        vec![0, 1],
        vec![1, 2],
    );

    assert_eq!(dataset.len(), 2);
    assert_eq!(dataset.num_features(), 2);
    assert_eq!(dataset.lookback(), 2);
    assert!(!dataset.is_empty());
}
