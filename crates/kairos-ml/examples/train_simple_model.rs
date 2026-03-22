//! # Simple Model Training Example
//!
//! This example demonstrates how to train a simple ML model using the
//! kairos-ml training pipeline.
//!
//! ## Running the Example
//!
//! ```bash
//! cargo run --example train_simple_model --release
//! ```
//!
//! ## Output
//!
//! The example will train a model and save it to `trained_model.pt`.
//! Training progress and metrics will be printed to the console.

use kairos_ml::training::training_loop::train;
use kairos_ml::training::{Dataset, LabelConfig, OptimizerType, TrainingConfig};
use std::path::Path;

/// Number of training samples
const NUM_SAMPLES: usize = 1000;
/// Lookback period for the model
const LOOKBACK: usize = 20;
/// Number of features
const NUM_FEATURES: usize = 3;

/// Generate synthetic training data
///
/// In a real application, this would load historical candle data
/// and compute indicator values from studies.
fn generate_synthetic_data(num_samples: usize) -> Dataset {
    let mut samples: Vec<Vec<Vec<f64>>> = Vec::new();
    let mut labels: Vec<usize> = Vec::new();
    let mut timestamps: Vec<i64> = Vec::new();

    for i in 0..num_samples {
        let t = i as f64;

        // Generate synthetic features with some patterns
        let mut sample_features: Vec<Vec<f64>> = Vec::new();
        let mut lookback_features: Vec<f64> = Vec::new();

        for _ in 0..LOOKBACK {
            let mut time_step_features: Vec<f64> = Vec::new();
            for f in 0..NUM_FEATURES {
                // Create correlated features with noise
                let base = (t * 0.01 + f as f64 * 0.5).sin() * 0.5;
                let noise = (t * 0.1 * (f + 1) as f64).sin() * 0.2;
                time_step_features.push(base + noise);
            }
            lookback_features.extend(time_step_features);
        }

        // Reshape features back to [lookback][features]
        for step in 0..LOOKBACK {
            let mut step_features: Vec<f64> = Vec::new();
            for f in 0..NUM_FEATURES {
                let idx = step * NUM_FEATURES + f;
                step_features.push(lookback_features[idx]);
            }
            sample_features.push(step_features);
        }

        samples.push(sample_features);

        // Generate labels based on a simple pattern
        // In real applications, labels would be derived from future returns
        let signal = if i % 3 == 0 {
            0 // Long
        } else if i % 3 == 1 {
            1 // Neutral
        } else {
            2 // Short
        };
        labels.push(signal);
        timestamps.push(i as i64);
    }

    Dataset::new(samples, labels, timestamps)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=================================================");
    println!("  Kairos ML - Simple Model Training Example");
    println!("=================================================");
    println!();

    // Create training configuration
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
        lstm_config: kairos_ml::training::config::LstmConfig::default(),
        gpu_device: None,
    };

    println!("Training Configuration:");
    println!("  Learning rate:   {}", config.learning_rate);
    println!("  Batch size:      {}", config.batch_size);
    println!("  Epochs:          {}", config.epochs);
    println!("  Optimizer:       {:?}", config.optimizer);
    println!("  Validation split: {}", config.validation_split);
    println!();

    // Generate synthetic training data
    println!("Generating synthetic training data...");
    let dataset = generate_synthetic_data(NUM_SAMPLES);

    println!("Dataset created:");
    println!("  Total samples:  {}", dataset.len());
    println!("  Lookback:       {}", dataset.lookback());
    println!("  Features:       {}", dataset.num_features());
    println!();

    // Create model
    let input_size = LOOKBACK * NUM_FEATURES;
    println!("Creating model...");
    println!("  Input size:  {}", input_size);
    println!("  Hidden size: 64");
    println!("  Output size: 3 (long, neutral, short)");

    println!();
    println!("Starting training...");
    println!("-------------------------------------------------");

    // Train the model
    let result = train(&config, &dataset, &LoggingCallback);

    println!("-------------------------------------------------");
    println!();
    println!("Training Complete!");
    println!("  Epochs trained:     {}", result.result.epochs_trained);
    println!("  Final train loss:   {:.4}", result.result.final_train_loss);
    if let Some(vl) = result.result.final_val_loss {
        println!("  Final val loss:     {:.4}", vl);
    }
    println!("  Early stopped:      {}", result.result.early_stopped);
    println!();

    // Save the trained model
    let output_path = Path::new("trained_model.pt");
    println!("Saving model to {}...", output_path.display());
    result.var_store.save(output_path)?;
    println!("Model saved successfully!");
    println!();

    // Print summary
    println!("=================================================");
    println!("  Training Summary");
    println!("=================================================");
    println!("  Model saved to:    {}", output_path.display());
    println!(
        "  Input shape:      [batch, {}] (lookback={}, features={})",
        input_size, LOOKBACK, NUM_FEATURES
    );
    println!("  Output shape:     [batch, 3] (long, neutral, short)");
    println!();
    println!("  To use this model in a backtest, run:");
    println!("    kairos ml validate-model --model trained_model.pt --data <sample.dbn>");
    println!();

    Ok(())
}

/// Training callback that prints progress
struct LoggingCallback;

impl kairos_ml::training::training_loop::TrainingCallback for LoggingCallback {
    fn on_epoch_end(&self, metrics: &kairos_ml::training::TrainingMetrics) -> bool {
        println!(
            "Epoch {:3}: train_loss={:.4}, val_loss={:+.4}, train_acc={:?}",
            metrics.epoch,
            metrics.train_loss,
            metrics.val_loss.unwrap_or(0.0) - metrics.train_loss,
            metrics.train_accuracy
        );
        true
    }
}
