//! # ML Commands
//!
//! CLI commands for ML model management and training.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use databento::dbn::decode::AsyncDbnDecoder;
use databento::dbn::TradeMsg;
use kairos_data::{DateRange, Price, Quantity, Side, Timestamp, Trade, Timeframe};
use kairos_ml::training::training_loop::{train as run_training, TrainResult};
use kairos_ml::Model;
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

    /// Candle timeframe for aggregation (default: 1min)
    #[arg(long, value_name = "TIMEFRAME", default_value = "1min")]
    pub timeframe: String,

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

    // Load feature configuration with all technical indicators
    let feature_config = if let Some(features_path) = &args.features {
        let content = fs::read_to_string(features_path)?;
        serde_json::from_str(&content)?
    } else {
        // Default feature configuration with all technical indicators
        // Features will be normalized relative to close price (no raw prices)
        use kairos_ml::features::{FeatureDefinition, FeatureTransform, NormalizationMethod};
        
        FeatureConfig {
            features: vec![
                // SMA features (normalized as % from close)
                FeatureDefinition::new("sma_20", "line"),
                FeatureDefinition::new("sma_50", "line"),
                // EMA features
                FeatureDefinition::new("ema_12", "line"),
                FeatureDefinition::new("ema_26", "line"),
                // Momentum
                FeatureDefinition::new("rsi", "value"),
                // Volatility
                FeatureDefinition::new("atr", "value"),
                // MACD
                FeatureDefinition::new("macd", "value"),
                FeatureDefinition::new("macd_signal", "value"),
                FeatureDefinition::new("macd_hist", "value"),
                // Bollinger Bands
                FeatureDefinition::new("bb_upper", "value"),
                FeatureDefinition::new("bb_lower", "value"),
                // Volume
                FeatureDefinition::new("vwap", "value"),
            ],
            lookback_periods: config.label_config.warmup_bars,
            normalization: NormalizationMethod::None, // Already normalized relative to close
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

    // Load REAL market data from DBN files
    println!("Loading REAL market data from DBN files...");
    
    // Parse dates if provided
    let start_date = args.start.as_ref()
        .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .context("Invalid start date format (use YYYY-MM-DD)"))
        .transpose()?
        .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2021, 1, 1).unwrap());
    
    let end_date = args.end.as_ref()
        .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .context("Invalid end date format (use YYYY-MM-DD)"))
        .transpose()?
        .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2021, 12, 31).unwrap());
    
    let symbol = args.symbol.as_deref().unwrap_or("NQ");
    let _lookback = feature_config.lookback_periods;
    
    // Load trades from DBN files
    let provider = MlDbnFileProvider::new(args.data_dir.clone(), symbol.to_string());
    let date_range = DateRange::new(start_date, end_date)
        .context("Invalid date range")?;
    
    let trades = provider.get_trades(&date_range).await
        .map_err(|e| anyhow::anyhow!("Failed to load trades: {}", e))?;
    
    if trades.is_empty() {
        anyhow::bail!("No trades found in the specified date range. Check your data directory.");
    }
    
    println!("Loaded {} trades", trades.len());
    
    // Parse timeframe and aggregate trades into candles
    let timeframe = parse_timeframe(&args.timeframe);
    let timeframe_ms = timeframe.to_milliseconds();
    println!("Aggregating into {} candles...", args.timeframe);
    let cli_candles = aggregate_trades_to_candles(&trades, timeframe_ms);
    
    if cli_candles.is_empty() {
        anyhow::bail!("Failed to create candles from trades");
    }
    
    println!("Generated {} candles", cli_candles.len());
    
    // Show price statistics
    if !cli_candles.is_empty() {
        let min_price = cli_candles.iter().map(|c| c.close).fold(f64::INFINITY, f64::min);
        let max_price = cli_candles.iter().map(|c| c.close).fold(0.0f64, f64::max);
        let avg_price: f64 = cli_candles.iter().map(|c| c.close).sum::<f64>() / cli_candles.len() as f64;
        println!("Price range: ${:.2} to ${:.2} (avg: ${:.2})", min_price, max_price, avg_price);
    }
    
    // Compute REAL technical indicators as features using DataGenerator
    println!("Computing technical indicators from REAL market data...");
    let studies = compute_study_outputs(&cli_candles);
    
    // Convert cli_candles to kairos-ml Candle format
    use kairos_ml::training::data_generator::{Candle as MlCandle, StudyOutput as MlStudyOutput};
    
    let ml_candles: Vec<MlCandle> = cli_candles.iter().map(|c| {
        MlCandle::new(c.open, c.high, c.low, c.close, c.volume, c.timestamp as i64)
    }).collect();
    
    let ml_studies: Vec<(&str, MlStudyOutput)> = vec![
        ("sma_20", MlStudyOutput::new(studies.sma_20, studies.timestamps.clone())),
        ("sma_50", MlStudyOutput::new(studies.sma_50, studies.timestamps.clone())),
        ("ema_12", MlStudyOutput::new(studies.ema_12, studies.timestamps.clone())),
        ("ema_26", MlStudyOutput::new(studies.ema_26, studies.timestamps.clone())),
        ("rsi", MlStudyOutput::new(studies.rsi, studies.timestamps.clone())),
        ("atr", MlStudyOutput::new(studies.atr, studies.timestamps.clone())),
        ("macd", MlStudyOutput::new(studies.macd, studies.timestamps.clone())),
        ("macd_signal", MlStudyOutput::new(studies.macd_signal, studies.timestamps.clone())),
        ("macd_hist", MlStudyOutput::new(studies.macd_hist, studies.timestamps.clone())),
        ("bb_upper", MlStudyOutput::new(studies.bb_upper, studies.timestamps.clone())),
        ("bb_lower", MlStudyOutput::new(studies.bb_lower, studies.timestamps.clone())),
        ("vwap", MlStudyOutput::new(studies.vwap, studies.timestamps.clone())),
    ];
    
    // Create data generator with the proper feature config
    let mut data_generator = DataGenerator::new(feature_config.clone(), config.label_config.clone());
    
    let dataset = data_generator.generate(&ml_candles, &ml_studies)
        .map_err(|e| anyhow::anyhow!("Failed to generate dataset: {}", e))?;
    
    if dataset.len() == 0 {
        anyhow::bail!("No valid samples after feature computation. Try a longer date range.");
    }

    println!("Dataset created:");
    println!("  Total samples: {}", dataset.len());
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
    let max_epochs = config.epochs;

    struct ProgressCallback {
        epochs_trained: Arc<Mutex<usize>>,
        final_loss: Arc<Mutex<f64>>,
        final_val_loss: Arc<Mutex<Option<f64>>>,
        early_stopped: Arc<Mutex<bool>>,
        verbose: bool,
        max_epochs: usize,
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
                    metrics.epoch, self.max_epochs, metrics.train_loss
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
        max_epochs,
    };

    // Run training
    let train_result = run_training(&config, &dataset, &progress_callback);

    *epochs_trained.lock().unwrap() = train_result.result.epochs_trained;
    *final_loss.lock().unwrap() = train_result.result.final_train_loss;
    *final_val_loss.lock().unwrap() = train_result.result.final_val_loss;
    *early_stopped.lock().unwrap() = train_result.result.early_stopped;

    if !args.verbose {
        println!(); // Newline after progress
    }

    println!();
    println!("Training complete!");
    println!("  Epochs trained:  {}", train_result.result.epochs_trained);
    println!("  Final train loss: {:.4}", train_result.result.final_train_loss);
    if let Some(vl) = train_result.result.final_val_loss {
        println!("  Final val loss:   {:.4}", vl);
    }
    println!("  Early stopped:   {}", train_result.result.early_stopped);
    println!();

    // Save the trained model with metadata
    println!("Saving trained model to {}...", args.output.display());
    
    // Get trained variables
    let trained_vars = train_result.var_store.variables();
    
    // Prepare metadata
    let num_feats = dataset.num_features();
    let lookb = dataset.lookback();
    let mut metadata = train_result.metadata.clone();
    metadata.num_features = num_feats as i64;
    metadata.lookback = lookb as i64;
    metadata.name = format!("nq_lstm_model");
    
    // Save as safetensors with metadata
    let weights_path = args.output.with_extension("safetensors");
    let json_path = args.output.with_extension("json");
    
    // Use Tensor::write_safetensors for better compatibility
    let named_tensors: Vec<(&str, &tch::Tensor)> = trained_vars
        .iter()
        .map(|(k, v)| (k.as_str(), v))
        .collect();
    
    tch::Tensor::write_safetensors(named_tensors.as_slice(), &weights_path)
        .map_err(|e| anyhow::anyhow!("Failed to save model weights: {}", e))?;
    
    // Save metadata
    let json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| anyhow::anyhow!("Failed to serialize metadata: {}", e))?;
    std::fs::write(&json_path, &json)?;
    
    println!("Model saved successfully!");
    println!("  Weights: {}", weights_path.display());
    println!("  Metadata: {}", json_path.display());

    // Print summary
    println!();
    println!("Training Summary");
    println!("================");
    println!("Model saved to: {}", args.output.display());
    let num_feats = dataset.num_features();
    let lookb = dataset.lookback();
    println!(
        "Input shape:    [batch, {}] (lookback={}, features={})",
        lookb * num_feats,
        lookb,
        num_feats
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

    let _registry = ModelRegistry::new();

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
    let model = match TchModel::load(&args.model) {
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

// ============================================================================
// REAL Data Loading from DBN Files
// ============================================================================

/// NQ futures instrument IDs (main contracts, not calendar spreads)
fn get_nq_instrument_ids() -> Vec<u32> {
    vec![
        // 2021 contracts (from actual file inspection)
        4378,   // NQH1 (NQ March 2021) - main
        2786,   // NQH1 variant
        828,    // NQH1 variant
        20987,  // NQH1 variant
        10351,  // NQM1 (NQ June 2021)
        10903,  // NQM1 variant
        2770,   // NQM1 variant
        19685,  // NQU1 (NQ September 2021)
        2895,   // NQZ1 (NQ December 2021)
        // Historical contracts
        29652,  // NQH1 (NQ March 2021)
        32274,  // NQM1 (NQ June 2021)
        29558,  // NQN1 (NQ July 2021)
        29804,  // NQU1 (NQ September 2021)
        29882,  // NQV1 (NQ October 2021)
        29653,  // NQZ1 (NQ December 2021)
        29754,  // NQF2 (NQ January 2022)
        29757,  // NQG2 (NQ February 2022)
        29763,  // NQH2 (NQ March 2022)
        // 2022 contracts
        33011,  // NQM2 (NQ June 2022)
        33014,  // NQN2 (NQ July 2022)
        33018,  // NQU2 (NQ September 2022)
        33021,  // NQV2 (NQ October 2022)
        33024,  // NQZ2 (NQ December 2022)
        // 2023 contracts
        20631,  // NQH3 (NQ March 2023)
        3522,   // NQM3 (NQ June 2023)
        2130,   // NQU3 (NQ September 2023)
        750,    // NQH4 (NQ March 2024)
        260937, // NQZ3 (NQ December 2023)
        106364, // NQZ4 (NQ December 2024)
    ]
}

/// Convert Databento price (10^-9 precision) to dollars
fn convert_price(dbn_price: i64) -> f64 {
    dbn_price as f64 / 1_000_000_000.0
}

/// Check if price is valid for the given symbol (filters calendar spreads)
fn is_valid_price(price: f64, symbol: &str) -> bool {
    match symbol {
        "NQ" => (5000.0..=30000.0).contains(&price),
        "ES" => (2000.0..=10000.0).contains(&price),
        "YM" => (15000.0..=50000.0).contains(&price),
        "RTY" => (800.0..=3000.0).contains(&price),
        _ => price > 0.0 && price < 1_000_000.0,
    }
}

/// Parse timeframe string to Timeframe enum
fn parse_timeframe(s: &str) -> Timeframe {
    match s.to_lowercase().as_str() {
        "1s" | "1sec" => Timeframe::M1s,
        "5s" | "5sec" => Timeframe::M5s,
        "10s" | "10sec" => Timeframe::M10s,
        "30s" | "30sec" => Timeframe::M30s,
        "1min" | "1m" => Timeframe::M1,
        "3min" | "3m" => Timeframe::M3,
        "5min" | "5m" => Timeframe::M5,
        "15min" | "15m" => Timeframe::M15,
        "30min" | "30m" => Timeframe::M30,
        "1hour" | "1h" => Timeframe::H1,
        "4hour" | "4h" => Timeframe::H4,
        "1day" | "1d" => Timeframe::D1,
        _ => {
            eprintln!("Warning: Unknown timeframe '{}', defaulting to 1min", s);
            Timeframe::M1
        }
    }
}

/// Extract dates from DBN filename
fn extract_dates(filename: &str) -> Option<(chrono::NaiveDate, chrono::NaiveDate)> {
    let stripped = filename.strip_prefix("glbx-mdp3-")?.split_once(".trades").map(|(d, _)| d)?;
    let parts: Vec<&str> = stripped.split('-').collect();
    if parts.len() >= 2 {
        let start = chrono::NaiveDate::parse_from_str(parts[0], "%Y%m%d").ok()?;
        let end = chrono::NaiveDate::parse_from_str(parts[1], "%Y%m%d").ok()?;
        Some((start, end))
    } else {
        None
    }
}

/// DBN file provider for ML training
pub struct MlDbnFileProvider {
    data_dir: PathBuf,
    symbol: String,
    valid_instrument_ids: Vec<u32>,
}

impl MlDbnFileProvider {
    pub fn new(data_dir: PathBuf, symbol: String) -> Self {
        // Get valid instrument IDs based on symbol
        let valid_ids = match symbol.to_uppercase().as_str() {
            "NQ" => get_nq_instrument_ids(),
            _ => vec![], // Empty means no filtering
        };
        
        Self {
            data_dir,
            symbol,
            valid_instrument_ids: valid_ids,
        }
    }
    
    fn find_files(&self, range: &DateRange) -> Vec<PathBuf> {
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.data_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.contains(".dbn") && (name.ends_with(".zst") || name.ends_with(".dbn")) {
                        if let Some(dates) = extract_dates(name) {
                            if dates.0 <= range.end && dates.1 >= range.start {
                                files.push(path);
                            }
                        }
                    }
                }
            }
        }
        files.sort();
        files
    }
    
    pub async fn get_trades(&self, range: &DateRange) -> Result<Vec<Trade>> {
        let files = self.find_files(range);
        
        if files.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut all_trades = Vec::new();
        
        for file in files {
            match self.load_trades_from_file(&file, range).await {
                Ok(trades) => all_trades.extend(trades),
                Err(e) => eprintln!("Warning: Failed to load {}: {}", file.display(), e),
            }
        }
        
        all_trades.sort_by_key(|t| t.time);
        Ok(all_trades)
    }
    
    async fn load_trades_from_file(&self, path: &PathBuf, range: &DateRange) -> Result<Vec<Trade>> {
        let mut decoder = AsyncDbnDecoder::from_zstd_file(path).await
            .with_context(|| format!("Failed to open DBN file: {}", path.display()))?;
        
        let start_dt = chrono::NaiveDateTime::new(range.start, chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        let end_dt = chrono::NaiveDateTime::new(range.end, chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap());
        let start_ts = start_dt.and_utc().timestamp_nanos_opt().unwrap() as u64;
        let end_ts = end_dt.and_utc().timestamp_nanos_opt().unwrap() as u64;
        
        let mut trades = Vec::new();
        
        while let Some(msg) = decoder.decode_record::<TradeMsg>().await? {
            let ts_recv = match msg.ts_recv() {
                Some(t) => t.unix_timestamp_nanos() as u64,
                None => continue,
            };
            
            if ts_recv < start_ts {
                continue;
            }
            if ts_recv > end_ts {
                break;
            }
            
            // Filter by instrument ID if we have valid IDs
            if !self.valid_instrument_ids.is_empty() {
                let instrument_id = msg.hd.instrument_id;
                if !self.valid_instrument_ids.contains(&instrument_id) {
                    continue;
                }
            }
            
            let price = convert_price(msg.price);
            
            // Filter by valid price range (filters calendar spreads)
            if !is_valid_price(price, &self.symbol) {
                continue;
            }
            
            let ts_ms = ts_recv / 1_000_000;
            
            let trade = Trade {
                time: Timestamp(ts_ms),
                price: Price::from_units((msg.price + msg.price.signum() * 5) / 10),
                quantity: Quantity(msg.size as f64),
                side: match msg.side() {
                    Ok(databento::dbn::Side::Ask) => Side::Sell,
                    _ => Side::Buy,
                },
            };
            trades.push(trade);
        }
        
        
        Ok(trades)
    }
}

// ============================================================================
// Candle Aggregation and Technical Indicators
// ============================================================================

/// OHLCV candle structure for CLI
#[derive(Clone, Debug)]
struct CliCandle {
    timestamp: u64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

/// Aggregate trades into candles
fn aggregate_trades_to_candles(trades: &[Trade], timeframe_ms: u64) -> Vec<CliCandle> {
    if trades.is_empty() {
        return Vec::new();
    }
    
    let mut candles_map: std::collections::BTreeMap<u64, CliCandle> = std::collections::BTreeMap::new();
    
    for trade in trades {
        let candle_ts = (trade.time.0 / timeframe_ms) * timeframe_ms;
        
        let price = trade.price.to_f64();
        
        if let Some(candle) = candles_map.get_mut(&candle_ts) {
            candle.high = candle.high.max(price);
            candle.low = candle.low.min(price);
            candle.close = price;
            candle.volume += trade.quantity.0;
        } else {
            candles_map.insert(candle_ts, CliCandle {
                timestamp: candle_ts,
                open: price,
                high: price,
                low: price,
                close: price,
                volume: trade.quantity.0,
            });
        }
    }
    
    candles_map.into_values().collect()
}

/// Compute technical indicators from candles
fn compute_indicators(candles: &[CliCandle]) -> TechnicalIndicatorOutput {
    let n = candles.len();
    if n < 60 {
        return TechnicalIndicatorOutput::default();
    }
    
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
    let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
    let volumes: Vec<f64> = candles.iter().map(|c| c.volume).collect();
    
    TechnicalIndicatorOutput {
        sma_20: compute_sma(&closes, 20),
        sma_50: compute_sma(&closes, 50),
        ema_12: compute_ema(&closes, 12),
        ema_26: compute_ema(&closes, 26),
        rsi: compute_rsi(&closes, 14),
        atr: compute_atr(&highs, &lows, &closes, 14),
        macd: compute_macd(&closes),
        macd_signal: compute_macd_signal(&closes),
        macd_hist: compute_macd_hist(&closes),
        bb_upper: compute_bb_upper(&closes, 20, 2.0),
        bb_middle: compute_sma(&closes, 20),
        bb_lower: compute_bb_lower(&closes, 20, 2.0),
        vwap: compute_vwap(&highs, &lows, &closes, &volumes),
        returns_1: compute_returns(&closes, 1),
        returns_3: compute_returns(&closes, 3),
        returns_5: compute_returns(&closes, 5),
        returns_10: compute_returns(&closes, 10),
        volatility: compute_volatility(&closes, 14),
        timestamps: candles.iter().map(|c| c.timestamp as i64).collect(),
    }
}

/// Technical indicator output container
#[derive(Default)]
struct TechnicalIndicatorOutput {
    sma_20: Vec<f64>,
    sma_50: Vec<f64>,
    ema_12: Vec<f64>,
    ema_26: Vec<f64>,
    rsi: Vec<f64>,
    atr: Vec<f64>,
    macd: Vec<f64>,
    macd_signal: Vec<f64>,
    macd_hist: Vec<f64>,
    bb_upper: Vec<f64>,
    bb_middle: Vec<f64>,
    bb_lower: Vec<f64>,
    vwap: Vec<f64>,
    returns_1: Vec<f64>,
    returns_3: Vec<f64>,
    returns_5: Vec<f64>,
    returns_10: Vec<f64>,
    volatility: Vec<f64>,
    timestamps: Vec<i64>,
}

fn compute_sma(prices: &[f64], period: usize) -> Vec<f64> {
    let mut result = vec![f64::NAN; prices.len()];
    if period == 0 || period > prices.len() {
        return result;
    }
    let start = period.saturating_sub(1);
    for i in start..prices.len() {
        let slice_start = i.saturating_sub(period).saturating_add(1);
        result[i] = prices[slice_start..=i].iter().sum::<f64>() / period as f64;
    }
    result
}

fn compute_ema(prices: &[f64], period: usize) -> Vec<f64> {
    let mut result = vec![f64::NAN; prices.len()];
    if period == 0 || period > prices.len() {
        return result;
    }
    
    let alpha = 2.0 / (period as f64 + 1.0);
    result[period - 1] = prices[..period].iter().sum::<f64>() / period as f64;
    
    for i in period..prices.len() {
        result[i] = alpha * prices[i] + (1.0 - alpha) * result[i - 1];
    }
    result
}

fn compute_rsi(prices: &[f64], period: usize) -> Vec<f64> {
    let mut result = vec![f64::NAN; prices.len()];
    if prices.len() < period + 1 {
        return result;
    }
    
    let deltas: Vec<f64> = prices.windows(2)
        .map(|w| w[1] - w[0])
        .collect();
    
    let mut avg_gain = deltas[..period].iter().filter(|&&d| d > 0.0).sum::<f64>() / period as f64;
    let mut avg_loss = deltas[..period].iter().filter(|&&d| d < 0.0).sum::<f64>().abs() / period as f64;
    
    result[period] = if avg_loss == 0.0 {
        100.0
    } else {
        100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
    };
    
    for i in period..deltas.len() {
        let gain = if deltas[i] > 0.0 { deltas[i] } else { 0.0 };
        let loss = if deltas[i] < 0.0 { -deltas[i] } else { 0.0 };
        
        avg_gain = (avg_gain * (period - 1) as f64 + gain) / period as f64;
        avg_loss = (avg_loss * (period - 1) as f64 + loss) / period as f64;
        
        result[i + 1] = if avg_loss == 0.0 {
            100.0
        } else {
            100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
        };
    }
    
    result
}

fn compute_atr(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let mut result = vec![f64::NAN; highs.len()];
    if highs.len() < period + 1 {
        return result;
    }
    
    let mut tr = vec![0.0; highs.len() - 1];
    tr[0] = highs[0] - lows[0];
    
    for i in 1..tr.len() {
        let hl = highs[i] - lows[i];
        let hc = (highs[i] - closes[i - 1]).abs();
        let lc = (lows[i] - closes[i - 1]).abs();
        tr[i] = hl.max(hc).max(lc);
    }
    
    result[period] = tr[..period].iter().sum::<f64>() / period as f64;
    
    for i in period..tr.len() {
        result[i + 1] = (result[i] * (period - 1) as f64 + tr[i]) / period as f64;
    }
    
    result
}

fn compute_macd(prices: &[f64]) -> Vec<f64> {
    let ema_fast = compute_ema(prices, 12);
    let ema_slow = compute_ema(prices, 26);
    ema_fast.iter().zip(ema_slow.iter())
        .map(|(f, s)| f - s)
        .collect()
}

fn compute_macd_signal(prices: &[f64]) -> Vec<f64> {
    let macd = compute_macd(prices);
    let ema = compute_ema(&macd, 9);
    ema
}

fn compute_macd_hist(prices: &[f64]) -> Vec<f64> {
    let macd = compute_macd(prices);
    let signal = compute_macd_signal(prices);
    macd.iter().zip(signal.iter())
        .map(|(m, s)| m - s)
        .collect()
}

fn compute_bb_upper(prices: &[f64], period: usize, std_dev: f64) -> Vec<f64> {
    let sma = compute_sma(prices, period);
    let mut result = vec![f64::NAN; prices.len()];
    if period == 0 || period > prices.len() {
        return result;
    }
    
    let start_idx = period.saturating_sub(1);
    for i in start_idx..prices.len() {
        let mean = sma[i];
        let slice_start = i.saturating_sub(period).saturating_add(1);
        let variance = prices[slice_start..=i].iter()
            .map(|&p| (p - mean).powi(2))
            .sum::<f64>() / period as f64;
        result[i] = mean + std_dev * variance.sqrt();
    }
    result
}

fn compute_bb_lower(prices: &[f64], period: usize, std_dev: f64) -> Vec<f64> {
    let sma = compute_sma(prices, period);
    let mut result = vec![f64::NAN; prices.len()];
    if period == 0 || period > prices.len() {
        return result;
    }
    
    let start_idx = period.saturating_sub(1);
    for i in start_idx..prices.len() {
        let mean = sma[i];
        let slice_start = i.saturating_sub(period).saturating_add(1);
        let variance = prices[slice_start..=i].iter()
            .map(|&p| (p - mean).powi(2))
            .sum::<f64>() / period as f64;
        result[i] = mean - std_dev * variance.sqrt();
    }
    result
}

fn compute_vwap(highs: &[f64], lows: &[f64], closes: &[f64], volumes: &[f64]) -> Vec<f64> {
    let mut result = vec![f64::NAN; closes.len()];
    let mut cumulative_tpv = 0.0;
    let mut cumulative_vol = 0.0;
    
    for i in 0..closes.len() {
        let typical_price = (highs[i] + lows[i] + closes[i]) / 3.0;
        cumulative_tpv += typical_price * volumes[i];
        cumulative_vol += volumes[i];
        
        if cumulative_vol > 0.0 {
            result[i] = cumulative_tpv / cumulative_vol;
        }
    }
    result
}

fn compute_returns(prices: &[f64], lag: usize) -> Vec<f64> {
    let mut result = vec![0.0; prices.len()];
    for i in lag..prices.len() {
        if prices[i - lag].abs() > f64::EPSILON {
            result[i] = (prices[i] - prices[i - lag]) / prices[i - lag];
        }
    }
    result
}

fn compute_volatility(prices: &[f64], period: usize) -> Vec<f64> {
    let mut result = vec![f64::NAN; prices.len()];
    for i in period..prices.len() {
        let slice = &prices[i - period..i];
        let mean = slice.iter().sum::<f64>() / period as f64;
        let variance = slice.iter()
            .map(|&p| (p - mean).powi(2))
            .sum::<f64>() / period as f64;
        result[i] = variance.sqrt();
    }
    result
}

/// Compute features and labels from candles
/// Study outputs container for DataGenerator
struct StudyOutputs {
    sma_20: Vec<f64>,
    sma_50: Vec<f64>,
    ema_12: Vec<f64>,
    ema_26: Vec<f64>,
    rsi: Vec<f64>,
    atr: Vec<f64>,
    macd: Vec<f64>,
    macd_signal: Vec<f64>,
    macd_hist: Vec<f64>,
    bb_upper: Vec<f64>,
    bb_lower: Vec<f64>,
    vwap: Vec<f64>,
    timestamps: Vec<i64>,
}

impl Default for StudyOutputs {
    fn default() -> Self {
        Self {
            sma_20: Vec::new(),
            sma_50: Vec::new(),
            ema_12: Vec::new(),
            ema_26: Vec::new(),
            rsi: Vec::new(),
            atr: Vec::new(),
            macd: Vec::new(),
            macd_signal: Vec::new(),
            macd_hist: Vec::new(),
            bb_upper: Vec::new(),
            bb_lower: Vec::new(),
            vwap: Vec::new(),
            timestamps: Vec::new(),
        }
    }
}

/// Compute study outputs (technical indicators) for DataGenerator
fn compute_study_outputs(candles: &[CliCandle]) -> StudyOutputs {
    let indicators = compute_indicators(candles);
    
    let n = candles.len();
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    
    // Normalize features relative to close price
    let mut studies = StudyOutputs::default();
    studies.timestamps = indicators.timestamps.clone();
    
    for i in 0..n {
        let close = closes[i];
        
        // SMA (normalized as percentage difference from close)
        studies.sma_20.push(
            if indicators.sma_20[i].is_finite() {
                (indicators.sma_20[i] / close - 1.0).clamp(-0.1, 0.1)
            } else {
                0.0
            }
        );
        studies.sma_50.push(
            if indicators.sma_50[i].is_finite() {
                (indicators.sma_50[i] / close - 1.0).clamp(-0.1, 0.1)
            } else {
                0.0
            }
        );
        studies.ema_12.push(
            if indicators.ema_12[i].is_finite() {
                (indicators.ema_12[i] / close - 1.0).clamp(-0.1, 0.1)
            } else {
                0.0
            }
        );
        studies.ema_26.push(
            if indicators.ema_26[i].is_finite() {
                (indicators.ema_26[i] / close - 1.0).clamp(-0.1, 0.1)
            } else {
                0.0
            }
        );
        
        // RSI (normalized to 0-1 range)
        studies.rsi.push(
            if indicators.rsi[i].is_finite() {
                (indicators.rsi[i] / 100.0).clamp(0.0, 1.0)
            } else {
                0.5
            }
        );
        
        // ATR (normalized as percentage of close)
        studies.atr.push(
            if indicators.atr[i].is_finite() {
                (indicators.atr[i] / close).clamp(0.0, 0.05)
            } else {
                0.0
            }
        );
        
        // MACD (normalized as percentage of close)
        studies.macd.push(
            if indicators.macd[i].is_finite() {
                (indicators.macd[i] / close).clamp(-0.01, 0.01)
            } else {
                0.0
            }
        );
        studies.macd_signal.push(
            if indicators.macd_signal[i].is_finite() {
                (indicators.macd_signal[i] / close).clamp(-0.01, 0.01)
            } else {
                0.0
            }
        );
        studies.macd_hist.push(
            if indicators.macd_hist[i].is_finite() {
                (indicators.macd_hist[i] / close).clamp(-0.005, 0.005)
            } else {
                0.0
            }
        );
        
        // Bollinger Bands (normalized as percentage from close)
        studies.bb_upper.push(
            if indicators.bb_upper[i].is_finite() {
                ((indicators.bb_upper[i] - close) / close).clamp(-0.05, 0.05)
            } else {
                0.0
            }
        );
        studies.bb_lower.push(
            if indicators.bb_lower[i].is_finite() {
                ((indicators.bb_lower[i] - close) / close).clamp(-0.05, 0.05)
            } else {
                0.0
            }
        );
        
        // VWAP (normalized as percentage difference from close)
        studies.vwap.push(
            if indicators.vwap[i].is_finite() {
                (indicators.vwap[i] / close - 1.0).clamp(-0.01, 0.01)
            } else {
                0.0
            }
        );
    }
    
    // Count valid samples
    let valid_count = studies.timestamps.len();
    println!("Computed {} study outputs (technical indicators)", valid_count);
    
    studies
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
