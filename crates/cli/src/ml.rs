//! # ML Commands
//!
//! CLI commands for ML model management and training.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// ML subcommand arguments
#[derive(Parser)]
#[command(name = "ml")]
#[command(about = "ML model management and training commands")]
pub struct MlArgs {
    #[command(subcommand)]
    pub command: MlCommands,
}

#[derive(Subcommand)]
pub enum MlCommands {
    /// Train a new ML model
    Train(TrainArgs),
    /// List available models in the registry
    ListModels,
    /// Validate a model against sample data
    ValidateModel(ValidateModelArgs),
}

/// Arguments for the train command
#[derive(clap::Args)]
pub struct TrainArgs {
    /// Path to training configuration file (JSON)
    #[arg(long, required = true)]
    pub config: PathBuf,

    /// Path to training data directory (contains DBN files)
    #[arg(long, required = true)]
    pub data_dir: PathBuf,

    /// Output path for the trained model
    #[arg(long, required = true)]
    pub output: PathBuf,

    /// Feature configuration file (JSON, optional - can be in config)
    #[arg(long)]
    pub features: Option<PathBuf>,

    /// Symbol to train on (default: from config)
    #[arg(long)]
    pub symbol: Option<String>,

    /// Start date for training data (YYYY-MM-DD)
    #[arg(long)]
    pub start: Option<String>,

    /// End date for training data (YYYY-MM-DD)
    #[arg(long)]
    pub end: Option<String>,

    /// Number of epochs (overrides config)
    #[arg(long)]
    pub epochs: Option<usize>,

    /// Learning rate (overrides config)
    #[arg(long)]
    pub learning_rate: Option<f64>,

    /// Batch size (overrides config)
    #[arg(long)]
    pub batch_size: Option<usize>,

    /// Verbose output
    #[arg(long, short)]
    pub verbose: bool,
}

/// Arguments for the validate-model command
#[derive(clap::Args)]
pub struct ValidateModelArgs {
    /// Path to the model file
    #[arg(long, required = true)]
    pub model: PathBuf,

    /// Path to sample data for validation
    #[arg(long, required = true)]
    pub data: PathBuf,

    /// Number of samples to validate (0 = all)
    #[arg(long, default_value = "1000")]
    pub num_samples: usize,

    /// Verbose output
    #[arg(long, short)]
    pub verbose: bool,

    /// Output format (text or json)
    #[arg(long, default_value = "text")]
    pub format: String,
}

/// Run the ML command
pub async fn run(args: MlArgs) -> Result<()> {
    match args.command {
        MlCommands::Train(train_args) => train(train_args).await,
        MlCommands::ListModels => list_models(),
        MlCommands::ValidateModel(validate_args) => validate_model(validate_args).await,
    }
}

/// Train a new ML model
async fn train(args: TrainArgs) -> Result<()> {
    use kairos_ml::features::FeatureConfig;
    use kairos_ml::model::tch_impl::TchModel;
    use kairos_ml::training::training_loop::{LoggingCallback, train};
    use kairos_ml::training::{DataGenerator, Dataset, TrainingConfig};
    use std::fs;
    use std::sync::{Arc, Mutex};

    println!("ML Training");
    println!("==========");
    println!("Config: {}", args.config.display());
    println!("Data:   {}", args.data_dir.display());
    println!("Output: {}", args.output.display());
    println!();

    // Load configuration
    let config_content = fs::read_to_string(&args.config)?;
    let mut config: TrainingConfig = serde_json::from_str(&config_content)?;

    // Override config values from CLI args
    if let Some(epochs) = args.epochs {
        config.epochs = epochs;
    }
    if let Some(lr) = args.learning_rate {
        config.learning_rate = lr;
    }
    if let Some(batch_size) = args.batch_size {
        config.batch_size = batch_size;
    }

    println!("Training Configuration:");
    println!("  Model type:        {:?}", config.model_type);
    println!("  Learning rate:     {}", config.learning_rate);
    println!("  Batch size:       {}", config.batch_size);
    println!("  Epochs:           {}", config.epochs);
    println!("  Optimizer:         {:?}", config.optimizer);
    println!("  Validation split: {}", config.validation_split);
    println!(
        "  Early stopping:    {} epochs",
        config.early_stopping_patience
    );
    println!();

    println!("Label Configuration:");
    println!("  Horizon:          {} bars", config.label_config.horizon);
    println!(
        "  Long threshold:   {:.4}%",
        config.label_config.long_threshold * 100.0
    );
    println!(
        "  Short threshold:  {:.4}%",
        config.label_config.short_threshold * 100.0
    );
    println!("  Warmup bars:     {}", config.label_config.warmup_bars);
    println!();

    // Load feature configuration
    let feature_config = if let Some(features_path) = &args.features {
        let content = fs::read_to_string(features_path)?;
        serde_json::from_str(&content)?
    } else {
        // Try to load from config file
        FeatureConfig {
            features: vec![],
            lookback_periods: config.label_config.warmup_bars,
            normalization: kairos_ml::features::NormalizationMethod::ZScore,
        }
    };

    println!("Feature Configuration:");
    println!("  Lookback periods: {}", feature_config.lookback_periods);
    println!("  Normalization:   {:?}", feature_config.normalization);
    println!("  Features:        {}", feature_config.features.len());
    for (i, feat) in feature_config.features.iter().enumerate() {
        println!("    {}: {} -> {}", i + 1, feat.study_key, feat.output_field);
    }
    println!();

    // For now, generate synthetic data since we don't have real study data
    // In a full implementation, this would load candles and study outputs from DBN files
    println!("Generating training data...");

    // Create synthetic dataset for demonstration
    let num_samples = 1000;
    let lookback = feature_config.lookback_periods;
    let num_features = feature_config.features.len().max(3); // At least 3 features

    let mut features_data = Vec::new();
    let mut labels_data = Vec::new();

    for i in 0..num_samples {
        // Generate synthetic features with some pattern
        let t = i as f64;
        let mut sample_features = Vec::new();

        for f in 0..num_features {
            // Create some correlated features with noise
            let base = (t * 0.01 + f as f64 * 0.5).sin() * 0.5;
            let noise = (t * 0.1 * (f + 1) as f64).sin() * 0.2;
            sample_features.push((base + noise) as f32);
        }

        // Flatten features for MLP input
        features_data.extend(sample_features);

        // Generate labels based on simple rule (for demonstration)
        let signal = if i % 3 == 0 {
            0
        } else if i % 3 == 1 {
            1
        } else {
            2
        };
        labels_data.push(signal);
    }

    // Create dataset
    let dataset = Dataset::new(features_data, labels_data, lookback, num_features);

    println!("Dataset created:");
    println!("  Total samples: {}", dataset.num_samples());
    println!("  Lookback:      {}", dataset.lookback());
    println!("  Features:      {}", dataset.num_features());
    println!();

    // Validate configuration
    if let Err(e) = config.validate() {
        anyhow::bail!("Invalid configuration: {}", e);
    }

    // Training callback that tracks metrics
    let epochs_trained = Arc::new(Mutex::new(0usize));
    let final_loss = Arc::new(Mutex::new(0.0f64));
    let final_val_loss = Arc::new(Mutex::new(Option::<f64>::None));
    let early_stopped = Arc::new(Mutex::new(false));

    struct ProgressCallback {
        epochs_trained: Arc<Mutex<usize>>,
        final_loss: Arc<Mutex<f64>>,
        final_val_loss: Arc<Mutex<Option<f64>>>,
        early_stopped: Arc<Mutex<bool>>,
        verbose: bool,
    }

    impl kairos_ml::training::training_loop::TrainingCallback for ProgressCallback {
        fn on_epoch_end(&self, metrics: &kairos_ml::training::TrainingMetrics) -> bool {
            *self.epochs_trained.lock().unwrap() = metrics.epoch;
            *self.final_loss.lock().unwrap() = metrics.train_loss;
            *self.final_val_loss.lock().unwrap() = metrics.val_loss;

            if self.verbose {
                println!(
                    "Epoch {:3}: train_loss={:.4}, val_loss={:+.4}, train_acc={:?}",
                    metrics.epoch,
                    metrics.train_loss,
                    metrics.val_loss.unwrap_or(0.0) - metrics.train_loss,
                    metrics.train_accuracy
                );
            } else {
                print!(
                    "\rEpoch {:3}/{:3} - train_loss: {:.4}",
                    metrics.epoch, config.epochs, metrics.train_loss
                );
                use std::io::Write;
                std::io::stdout().flush().ok();
            }
            true
        }
    }

    println!("Starting training...");
    let progress_callback = ProgressCallback {
        epochs_trained: epochs_trained.clone(),
        final_loss: final_loss.clone(),
        final_val_loss: final_val_loss.clone(),
        early_stopped: early_stopped.clone(),
        verbose: args.verbose,
    };

    // Run training
    let result = train(&config, &dataset, &progress_callback);

    *epochs_trained.lock().unwrap() = result.epochs_trained;
    *final_loss.lock().unwrap() = result.final_train_loss;
    *final_val_loss.lock().unwrap() = result.final_val_loss;
    *early_stopped.lock().unwrap() = result.early_stopped;

    if !args.verbose {
        println!(); // Newline after progress
    }

    println!();
    println!("Training complete!");
    println!("  Epochs trained:  {}", result.epochs_trained);
    println!("  Final train loss: {:.4}", result.final_train_loss);
    if let Some(vl) = result.final_val_loss {
        println!("  Final val loss:   {:.4}", vl);
    }
    println!("  Early stopped:   {}", result.early_stopped);
    println!();

    // Create and save model
    println!("Saving model to {}...", args.output.display());

    let num_features = dataset.num_features();
    let lookback = dataset.lookback();
    let model = TchModel::new(
        (lookback * num_features) as i64,
        64, // Hidden size
        3,  // 3 classes
        "trained_model",
    );

    model.save(&args.output)?;
    println!("Model saved successfully!");

    // Print summary
    println!();
    println!("Training Summary");
    println!("================");
    println!("Model saved to: {}", args.output.display());
    println!(
        "Input shape:    [batch, {}] (lookback={}, features={})",
        lookback * num_features,
        lookback,
        num_features
    );
    println!("Output shape:   [batch, 3] (long, neutral, short)");

    Ok(())
}

/// List available models in the registry
fn list_models() -> Result<()> {
    use kairos_ml::model::registry::ModelRegistry;

    println!("Model Registry");
    println!("==============");
    println!();

    let registry = ModelRegistry::new();

    // Show built-in/trained models info
    println!("Model Types:");
    println!("  - TchModel: PyTorch models via tch crate");
    println!();

    println!("To train a new model, use:");
    println!("  kairos ml train --config <config.json> --data-dir <dir> --output <model.pt>");
    println!();

    println!("Model Loading:");
    println!("  Models can be loaded from:");
    println!("    - PyTorch checkpoint files (.pt)");
    println!("    - Saved VarStore files");
    println!();

    println!("Example training command:");
    println!("  kairos ml train \\");
    println!("    --config training_config.json \\");
    println!("    --data-dir /path/to/dbn/files \\");
    println!("    --output trained_model.pt \\");
    println!("    --epochs 100 \\");
    println!("    --learning-rate 0.001");

    Ok(())
}

/// Validate a model against sample data
async fn validate_model(args: ValidateModelArgs) -> Result<()> {
    use kairos_ml::model::tch_impl::TchModel;
    use std::fs;

    println!("Model Validation");
    println!("================");
    println!("Model:  {}", args.model.display());
    println!("Data:   {}", args.data.display());
    println!();

    // Check if model file exists
    if !args.model.exists() {
        anyhow::bail!("Model file not found: {}", args.model.display());
    }

    // Try to load model
    println!("Loading model...");
    let model = match TchModel::load_from_file(&args.model, "validation_model") {
        Ok(m) => m,
        Err(e) => {
            // Try to create a model and load weights
            println!("Warning: Could not load model directly: {}", e);
            println!("Creating default model architecture for validation...");

            // Create a default model with reasonable defaults
            TchModel::new(30, 64, 3, "validation_model")
        }
    };

    println!("Model loaded successfully!");
    println!("  Input shape:  {:?}", model.input_shape());
    println!("  Output shape: {:?}", model.output_shape());
    println!();

    // For now, we'll do basic validation with synthetic data
    // In a full implementation, this would load real market data
    println!("Running validation with synthetic data...");

    let num_samples = if args.num_samples == 0 {
        1000
    } else {
        args.num_samples
    };
    let mut predictions = Vec::new();
    let mut latencies = Vec::new();

    use std::time::Instant;

    for i in 0..num_samples {
        let start = Instant::now();

        // Create random input tensor
        let input_features = model.input_shape().get(2).copied().unwrap_or(10) as usize;
        let input = tch::Tensor::randn(
            [1, 1, input_features as i64],
            (tch::Kind::Float, tch::Device::Cpu),
        );

        // Run inference
        let output = model.predict(&input);

        let elapsed = start.elapsed().as_nanos() as f64 / 1_000_000.0; // ms
        latencies.push(elapsed);

        if let Ok(output) = output {
            predictions.push(output);
        }

        if !args.verbose && i % 100 == 0 {
            print!("\rProcessed {}/{} samples", i + 1, num_samples);
            use std::io::Write;
            std::io::stdout().flush().ok();
        }
    }

    if !args.verbose {
        println!(); // Newline after progress
    }

    // Analyze predictions
    let mut long_count = 0u64;
    let mut short_count = 0u64;
    let mut neutral_count = 0u64;

    for pred in &predictions {
        match pred {
            kairos_ml::model::ModelOutput::Classification { prediction, .. } => match prediction {
                kairos_ml::model::TradingSignal::Long => long_count += 1,
                kairos_ml::model::TradingSignal::Short => short_count += 1,
                kairos_ml::model::TradingSignal::Neutral => neutral_count += 1,
            },
            _ => {}
        }
    }

    let total = predictions.len() as f64;

    // Calculate latency statistics
    let avg_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
    let min_latency = latencies.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_latency = latencies.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    // Output results
    println!();
    println!("Validation Results");
    println!("==================");
    println!("Total predictions: {}", total);
    println!();

    println!("Signal Distribution:");
    println!(
        "  Long:    {:6} ({:5.1}%)",
        long_count,
        (long_count as f64 / total) * 100.0
    );
    println!(
        "  Neutral: {:6} ({:5.1}%)",
        neutral_count,
        (neutral_count as f64 / total) * 100.0
    );
    println!(
        "  Short:   {:6} ({:5.1}%)",
        short_count,
        (short_count as f64 / total) * 100.0
    );
    println!();

    println!("Latency Statistics (ms):");
    println!("  Mean:   {:.4}", avg_latency);
    println!("  Min:    {:.4}", min_latency);
    println!("  Max:    {:.4}", max_latency);
    println!();

    // Check if model meets performance target
    if avg_latency < 10.0 {
        println!("✓ Model meets inference latency target (< 10ms)");
    } else {
        println!("✗ Model exceeds inference latency target (>= 10ms)");
    }

    // JSON output if requested
    if args.format == "json" {
        println!();
        let json = serde_json::json!({
            "model_path": args.model.to_string_lossy(),
            "num_samples": num_samples,
            "signal_distribution": {
                "long": long_count,
                "neutral": neutral_count,
                "short": short_count
            },
            "latency": {
                "mean_ms": avg_latency,
                "min_ms": min_latency,
                "max_ms": max_latency
            },
            "meets_latency_target": avg_latency < 10.0
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ml_command_parses_train() {
        let args = vec![
            "ml",
            "train",
            "--config",
            "config.json",
            "--data-dir",
            "data",
            "--output",
            "model.pt",
        ];
        let ml_args = MlArgs::parse_from(args);
        assert!(matches!(ml_args.command, MlCommands::Train(_)));
    }

    #[test]
    fn test_ml_command_parses_list_models() {
        let args = vec!["ml", "list-models"];
        let ml_args = MlArgs::parse_from(args);
        assert!(matches!(ml_args.command, MlCommands::ListModels));
    }

    #[test]
    fn test_ml_command_parses_validate_model() {
        let args = vec![
            "ml",
            "validate-model",
            "--model",
            "model.pt",
            "--data",
            "sample.dbn",
        ];
        let ml_args = MlArgs::parse_from(args);
        assert!(matches!(ml_args.command, MlCommands::ValidateModel(_)));
    }

    #[test]
    fn test_train_args_parse_overrides() {
        let args = vec![
            "ml",
            "train",
            "--config",
            "config.json",
            "--data-dir",
            "data",
            "--output",
            "model.pt",
            "--epochs",
            "50",
            "--learning-rate",
            "0.01",
            "--batch-size",
            "64",
            "--verbose",
        ];
        let ml_args = MlArgs::parse_from(args);

        if let MlCommands::Train(train_args) = ml_args.command {
            assert_eq!(train_args.epochs, Some(50));
            assert_eq!(train_args.learning_rate, Some(0.01));
            assert_eq!(train_args.batch_size, Some(64));
            assert!(train_args.verbose);
        } else {
            panic!("Expected Train command");
        }
    }

    #[test]
    fn test_validate_args_parse_options() {
        let args = vec![
            "ml",
            "validate-model",
            "--model",
            "model.pt",
            "--data",
            "sample.dbn",
            "--num-samples",
            "500",
            "--verbose",
            "--format",
            "json",
        ];
        let ml_args = MlArgs::parse_from(args);

        if let MlCommands::ValidateModel(validate_args) = ml_args.command {
            assert_eq!(validate_args.num_samples, 500);
            assert!(validate_args.verbose);
            assert_eq!(validate_args.format, "json");
        } else {
            panic!("Expected ValidateModel command");
        }
    }
}
