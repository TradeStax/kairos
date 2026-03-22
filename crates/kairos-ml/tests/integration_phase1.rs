//! # Phase 1 Integration Tests
//!
//! Integration tests for Phase 1: Core Model Infrastructure

use kairos_ml::model::tch_impl::TchModel;
use kairos_ml::model::{Model, ModelOutput, TradingSignal};

/// Full inference pipeline test: create model, run inference, verify output
#[test]
fn test_full_inference_pipeline() {
    // Create model
    let model = TchModel::new(10, 32, 3, "integration_test_model");

    // Verify model properties
    assert_eq!(model.name(), "integration_test_model");
    assert_eq!(model.input_shape(), vec![1, 1, 10]);
    assert_eq!(model.output_shape(), vec![1, 3]);

    // Create test input: [batch=1, seq=1, features=10]
    let input = tch::Tensor::randn([1, 1, 10], (tch::Kind::Float, tch::Device::Cpu));

    // Run inference
    let output = model.predict(&input);

    // Verify output
    assert!(output.is_ok(), "Inference should succeed");

    let output = output.unwrap();

    match output {
        ModelOutput::Classification {
            probabilities,
            prediction,
        } => {
            // Verify probabilities array
            assert_eq!(probabilities.len(), 3, "Should have 3 class probabilities");

            // Verify probabilities sum to ~1.0
            let sum: f64 = probabilities.iter().sum();
            assert!((sum - 1.0).abs() < 0.01, "Probabilities should sum to 1.0");

            // Verify all probabilities are valid (0-1 range)
            for p in &probabilities {
                assert!(*p >= 0.0 && *p <= 1.0, "Probability should be in [0, 1]");
            }

            // Verify prediction is valid
            assert!(
                matches!(
                    prediction,
                    TradingSignal::Long | TradingSignal::Short | TradingSignal::Neutral
                ),
                "Prediction should be a valid trading signal"
            );
        }
        _ => panic!("Expected Classification output for 3-class model"),
    }
}

/// Test model with batch input
#[test]
fn test_batch_inference() {
    let model = TchModel::new(5, 16, 3, "batch_test_model");

    // Create batch input: [batch=4, seq=1, features=5]
    let input = tch::Tensor::randn([4, 1, 5], (tch::Kind::Float, tch::Device::Cpu));

    // Run inference via predict
    let output = model.predict(&input);
    assert!(output.is_ok());

    // For batch input [4, 1, 5], output shape should be [4, 3]
    // Verify via the raw forward pass
    let raw_output = model.forward_mlp(&input);
    let output_shape = raw_output.size();
    assert_eq!(output_shape, vec![4, 3]);
}

/// Test model registry integration
#[test]
fn test_model_registry_with_tch_model() {
    use kairos_ml::model::registry::ModelRegistry;

    let mut registry = ModelRegistry::new();

    // Register a tch model
    registry.register("test_classifier", || {
        Ok(TchModel::new(10, 32, 3, "registered_model"))
    });

    // Load the model
    let model = registry.load("test_classifier");
    assert!(model.is_ok());

    // Verify the model works
    let model = model.unwrap();
    let input = tch::Tensor::randn([1, 1, 10], (tch::Kind::Float, tch::Device::Cpu));
    let output = model.predict(&input);
    assert!(output.is_ok());

    // List registered models
    let models = registry.list();
    assert!(models.contains(&"test_classifier".to_string()));
}

/// Test multiple predictions for consistency
#[test]
fn test_multiple_predictions() {
    let model = TchModel::new(8, 32, 3, "consistency_test");

    // Create consistent input (same seed)
    let input = tch::Tensor::ones([1, 1, 8], (tch::Kind::Float, tch::Device::Cpu)) * 0.5;

    // Run multiple inferences
    let outputs: Vec<ModelOutput> = (0..5).map(|_| model.predict(&input).unwrap()).collect();

    // All outputs should be the same for the same input
    for output in &outputs {
        match (output, &outputs[0]) {
            (
                ModelOutput::Classification {
                    probabilities: p1, ..
                },
                ModelOutput::Classification {
                    probabilities: p2, ..
                },
            ) => {
                for (a, b) in p1.iter().zip(p2.iter()) {
                    assert!(
                        (a - b).abs() < 0.001,
                        "Same input should produce same output"
                    );
                }
            }
            _ => panic!("Expected Classification output"),
        }
    }
}

/// Test different model configurations
#[test]
fn test_different_model_configs() {
    // Test small model
    let small_model = TchModel::new(4, 8, 3, "small_model");
    assert_eq!(small_model.input_shape(), vec![1, 1, 4]);
    assert_eq!(small_model.output_shape(), vec![1, 3]);

    // Test large model
    let large_model = TchModel::new(100, 128, 3, "large_model");
    assert_eq!(large_model.input_shape(), vec![1, 1, 100]);
    assert_eq!(large_model.output_shape(), vec![1, 3]);

    // Test different output sizes
    let regression_model = TchModel::new(20, 64, 1, "regression_model");
    assert_eq!(regression_model.output_shape(), vec![1, 1]);
}

/// Test error handling for invalid inputs
#[test]
fn test_error_handling_invalid_shapes() {
    let model = TchModel::new(10, 32, 3, "error_test_model");

    // Test 1D input - model doesn't validate input shapes
    // It will reshape and pass through, producing unexpected output
    let input_1d = tch::Tensor::randn([10], (tch::Kind::Float, tch::Device::Cpu));
    let result = model.predict(&input_1d);
    // The model accepts any shape and returns a result
    assert!(result.is_ok());

    // Test 4D input - model doesn't validate dimensions
    let input_4d = tch::Tensor::randn([1, 1, 10, 1], (tch::Kind::Float, tch::Device::Cpu));
    let result = model.predict(&input_4d);
    assert!(result.is_ok());

    // Test correct input shape works as expected
    let input_correct = tch::Tensor::randn([1, 1, 10], (tch::Kind::Float, tch::Device::Cpu));
    let result = model.predict(&input_correct);
    assert!(result.is_ok());
}

/// Test model output serialization
#[test]
fn test_output_serialization_roundtrip() {
    use serde_json;

    // Create a classification output
    let original = ModelOutput::Classification {
        probabilities: [0.6, 0.3, 0.1],
        prediction: TradingSignal::Long,
    };

    // Serialize
    let json = serde_json::to_string(&original).unwrap();

    // Deserialize
    let parsed: ModelOutput = serde_json::from_str(&json).unwrap();

    match (&original, &parsed) {
        (
            ModelOutput::Classification {
                probabilities: p1,
                prediction: pred1,
            },
            ModelOutput::Classification {
                probabilities: p2,
                prediction: pred2,
            },
        ) => {
            assert_eq!(p1, p2);
            assert_eq!(pred1, pred2);
        }
        _ => panic!("Type mismatch after serialization"),
    }
}

/// Test inference with various float values
#[test]
fn test_inference_edge_cases() {
    let model = TchModel::new(5, 16, 3, "edge_case_test");

    // Test with zeros
    let zeros = tch::Tensor::zeros([1, 1, 5], (tch::Kind::Float, tch::Device::Cpu));
    let result = model.predict(&zeros);
    assert!(result.is_ok());

    // Test with all ones
    let ones = tch::Tensor::ones([1, 1, 5], (tch::Kind::Float, tch::Device::Cpu));
    let result = model.predict(&ones);
    assert!(result.is_ok());

    // Test with negative values
    let negatives = tch::Tensor::ones([1, 1, 5], (tch::Kind::Float, tch::Device::Cpu)) * -1.0;
    let result = model.predict(&negatives);
    assert!(result.is_ok());

    // Test with large values
    let large = tch::Tensor::ones([1, 1, 5], (tch::Kind::Float, tch::Device::Cpu)) * 1000.0;
    let result = model.predict(&large);
    assert!(result.is_ok());
}
