//! Backtest command

use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;
use std::sync::Arc;

use databento::dbn::decode::AsyncDbnDecoder;
use databento::dbn::TradeMsg;

use kairos_data::{DateRange, FuturesTicker, FuturesVenue, Price, Quantity, Side, Timestamp, Trade};
use kairos_backtest::config::backtest::BacktestConfig;
use kairos_backtest::BacktestRunner;
use kairos_backtest::output::export::BacktestExport;
use kairos_backtest::strategy::registry::StrategyRegistry;
use kairos_backtest::TradeProvider;

#[derive(Args)]
pub struct BacktestArgs {
    #[arg(short, long)]
    pub symbol: String,

    #[arg(long)]
    pub start: String,

    #[arg(long)]
    pub end: String,

    #[arg(long, default_value = "orb")]
    pub strategy: String,

    #[arg(long, default_value = "1min")]
    pub timeframe: String,

    #[arg(long, default_value = "100000")]
    pub capital: f64,

    #[arg(long, short)]
    pub verbose: bool,

    #[arg(long, default_value = "text")]
    pub format: String,

    #[arg(long)]
    pub export: Option<PathBuf>,

    #[arg(long, required = true)]
    pub data_dir: PathBuf,

    /// Path to trained ML model (required for ml strategy)
    #[arg(long)]
    pub model_path: Option<PathBuf>,

    /// Path to ML strategy config JSON (optional, uses defaults if not provided)
    #[arg(long)]
    pub strategy_config: Option<PathBuf>,
}

fn parse_timeframe(s: &str) -> kairos_data::Timeframe {
    match s.to_lowercase().as_str() {
        "1min" | "1m" => kairos_data::Timeframe::M1,
        "5min" | "5m" => kairos_data::Timeframe::M5,
        "15min" | "15m" => kairos_data::Timeframe::M15,
        "1hour" | "1h" => kairos_data::Timeframe::H1,
        "1day" | "1d" => kairos_data::Timeframe::D1,
        _ => kairos_data::Timeframe::M1,
    }
}

/// Convert Databento price (10^-9 precision) to Price (10^-8 precision)
fn convert_price(dbn_price: i64) -> Price {
    Price::from_units((dbn_price + dbn_price.signum() * 5) / 10)
}

/// Valid price ranges for futures (in dollars)
/// Prices outside these ranges indicate bad data or different instruments
fn is_valid_price(price: f64, symbol: &str) -> bool {
    let min_price = match symbol {
        "NQ" => 5000.0,
        "ES" => 2000.0,
        "YM" => 15000.0,
        "RTY" => 800.0,
        "GC" => 1000.0,
        "CL" => 30.0,
        _ => 100.0,
    };
    
    let max_price = match symbol {
        "NQ" => 30000.0,
        "ES" => 10000.0,
        "YM" => 50000.0,
        "RTY" => 3000.0,
        "GC" => 3000.0,
        "CL" => 200.0,
        _ => 1_000_000.0,
    };
    
    price >= min_price && price <= max_price
}

/// Get the instrument IDs for main NQ contracts (not spreads)
/// Calendar spreads have prices that are differences, not absolute prices
fn get_nq_instrument_ids() -> Vec<u32> {
    vec![
        // 2021 contracts
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

pub async fn run(args: BacktestArgs) -> Result<()> {
    let start_date = chrono::NaiveDate::parse_from_str(&args.start, "%Y-%m-%d")
        .with_context(|| format!("Invalid start date: {}", args.start))?;
    let end_date = chrono::NaiveDate::parse_from_str(&args.end, "%Y-%m-%d")
        .with_context(|| format!("Invalid end date: {}", args.end))?;

    let timeframe = parse_timeframe(&args.timeframe);
    let symbol_upper = args.symbol.to_uppercase();
    
    let ticker = match symbol_upper.as_str() {
        "NQ" => FuturesTicker::new("NQ.c.0", FuturesVenue::CMEGlobex),
        "ES" => FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex),
        sym => FuturesTicker::new(&format!("{}.c.0", sym), FuturesVenue::CMEGlobex),
    };

    let date_range = DateRange::new(start_date, end_date)
        .context("Invalid date range")?;

    // Handle ML strategy differently
    let strategy: Box<dyn kairos_backtest::Strategy> = if args.strategy == "ml" {
        create_ml_strategy(&args, &ticker).await?
    } else {
        let registry = StrategyRegistry::with_built_ins();

        if !registry.contains(&args.strategy) {
            anyhow::bail!("Unknown strategy: {}. Use 'orb', 'vwap_reversion', 'momentum_breakout', or 'ml'", args.strategy);
        }

        registry.create(&args.strategy)
            .with_context(|| format!("Failed to create strategy: {}", args.strategy))?
    };

    let mut config = BacktestConfig::default_es(&args.strategy);
    config.ticker = ticker;
    config.date_range = date_range.clone();
    config.timeframe = timeframe;
    config.initial_capital_usd = args.capital;
    config.warm_up_periods = if args.strategy == "ml" { 100 } else { 50 };

    println!("Kairos Backtest");
    println!("Symbol: {} | Period: {} to {} | Strategy: {}", 
             args.symbol, start_date, end_date, args.strategy);
    println!("Initial Capital: ${:.2}", args.capital);

    // Create provider
    let provider = Arc::new(DbnFileProvider::new(args.data_dir.clone(), symbol_upper.clone()));
    
    println!("Loading trades...");
    let trades = provider.get_trades(&ticker, &config.date_range).await
        .map_err(|e| anyhow::anyhow!("Failed to load trades: {}", e))?;
    
    println!("Loaded {} trades", trades.len());

    if trades.is_empty() {
        println!("No trades found.");
        return Ok(());
    }

    // Analyze price data
    if !trades.is_empty() {
        let min_price = trades.iter().map(|t| t.price.units()).min().unwrap();
        let max_price = trades.iter().map(|t| t.price.units()).max().unwrap();
        let avg_price = trades.iter().map(|t| t.price.units() as f64).sum::<f64>() / trades.len() as f64;
        println!("Price range: ${:.2} to ${:.2} (avg: ${:.2})", 
                 min_price as f64 / 100_000_000.0,
                 max_price as f64 / 100_000_000.0,
                 avg_price / 100_000_000.0);
    }

    println!("Running backtest...");
    let runner = BacktestRunner::new(provider);
    let result = runner.run(config, strategy).await
        .map_err(|e| anyhow::anyhow!("Backtest failed: {}", e))?;

    // Create the export for JSON output and file export
    let export = BacktestExport::from_result(&result);

    // Handle JSON format or export
    if args.format == "json" || args.export.is_some() {
        let json = serde_json::to_string_pretty(&export)?;
        
        if args.format == "json" {
            println!("{}", json);
        }
        
        if let Some(export_path) = &args.export {
            std::fs::write(export_path, &json)?;
            println!("\nExport saved to: {}", export_path.display());
        }
    }

    // Text output (default)
    if args.format != "json" {
        println!("\nResults");
        println!("=======");
        println!("Final Equity: ${:.2}", result.metrics.final_equity_usd);
        println!("Return: {:.2}%", result.metrics.total_return_pct);
        println!("Max Drawdown: {:.2}% (${:.2})", 
                 result.metrics.max_drawdown_pct, result.metrics.max_drawdown_usd);
        println!("Trades: {}", result.metrics.total_trades);
        println!("Win Rate: {:.1}%", result.metrics.win_rate * 100.0);
        println!("Profit Factor: {:.2}", result.metrics.profit_factor);
        println!("Sharpe: {:.2}", result.metrics.sharpe_ratio);
        println!("Sortino: {:.2}", result.metrics.sortino_ratio);
        
        if !result.warnings.is_empty() {
            println!("\nWarnings:");
            for warning in &result.warnings {
                println!("  - {}", warning);
            }
        }
    }

    // Verbose output (only in text mode)
    if args.verbose && args.format != "json" {
        println!("\nEquity Curve Points: {}", result.equity_curve.points.len());
        if !result.equity_curve.points.is_empty() {
            let min_eq = result.equity_curve.points.iter()
                .map(|p| p.total_equity_usd).fold(f64::INFINITY, f64::min);
            let max_eq = result.equity_curve.points.iter()
                .map(|p| p.total_equity_usd).fold(f64::NEG_INFINITY, f64::max);
            let initial = result.equity_curve.initial_equity_usd;
            println!("Initial equity: ${:.2}", initial);
            println!("Equity range: ${:.2} to ${:.2}", min_eq, max_eq);
        }
        
        println!("\nFirst 10 trades:");
        for (i, trade) in result.trades.iter().enumerate().take(10) {
            let entry_dollars = trade.entry_price.to_f64();
            let exit_dollars = trade.exit_price.to_f64();
            println!("  Trade {}: {:?} {} @ entry ${:.2} exit ${:.2} P&L: ${:.2} | {:?}", 
                     i + 1, trade.side, trade.quantity, 
                     entry_dollars, exit_dollars, trade.pnl_net_usd, trade.exit_reason);
        }
    }

    Ok(())
}

pub struct DbnFileProvider {
    data_dir: PathBuf,
    symbol: String,
}

impl DbnFileProvider {
    pub fn new(data_dir: PathBuf, symbol: String) -> Self {
        Self { data_dir, symbol }
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
}

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

impl TradeProvider for DbnFileProvider {
    fn get_trades(
        &self,
        _ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Trade>, String>> + Send + '_>> {
        let data_dir = self.data_dir.clone();
        let symbol = self.symbol.clone();
        let range = date_range.clone();
        Box::pin(async move {
            let files = Self::find_files_internal(&data_dir, &range);
            if files.is_empty() {
                return Ok(Vec::new());
            }

            let mut all_trades = Vec::new();
            let mut filtered_count = 0;
            
            // Get valid instrument IDs for main NQ contracts (not spreads)
            // For now, we'll just use price filtering instead of instrument IDs
            let valid_instrument_ids: Vec<u32> = Vec::new();
            
            for file in files {
                match load_trades_from_file(&file, &range, &symbol, &valid_instrument_ids).await {
                    Ok(trades) => {
                        for trade in trades {
                            let price_dollars = trade.price.to_f64();
                            if is_valid_price(price_dollars, &symbol) {
                                all_trades.push(trade);
                            } else {
                                filtered_count += 1;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load {}: {}", file.display(), e);
                    }
                }
            }

            if filtered_count > 0 {
                eprintln!("Filtered {} trades (calendar spreads or invalid prices)", filtered_count);
            }

            all_trades.sort_by_key(|t| t.time);
            Ok(all_trades)
        })
    }
}

impl DbnFileProvider {
    fn find_files_internal(data_dir: &PathBuf, range: &DateRange) -> Vec<PathBuf> {
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(data_dir) {
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
}

async fn load_trades_from_file(
    path: &PathBuf, 
    range: &DateRange, 
    symbol: &str,
    valid_instrument_ids: &[u32],
) -> anyhow::Result<Vec<Trade>> {
    let mut decoder = AsyncDbnDecoder::from_zstd_file(path).await
        .with_context(|| format!("Failed to open DBN file: {}", path.display()))?;

    let mut trades = Vec::new();
    
    let start_dt = chrono::NaiveDateTime::new(range.start, chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    let end_dt = chrono::NaiveDateTime::new(range.end, chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap());
    let start_ts = start_dt.and_utc().timestamp_nanos_opt().unwrap() as u64;
    let end_ts = end_dt.and_utc().timestamp_nanos_opt().unwrap() as u64;

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

        let ts_ms = ts_recv / 1_000_000;
        
        // Filter by instrument ID if we have valid IDs
        // This filters out calendar spreads which have different instrument IDs
        if !valid_instrument_ids.is_empty() {
            let instrument_id = msg.hd.instrument_id;
            if !valid_instrument_ids.contains(&instrument_id) {
                continue;
            }
        }

        let trade = Trade {
            time: Timestamp(ts_ms),
            price: convert_price(msg.price),
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

// ============================================================================
// ML Strategy Support
// ============================================================================

use kairos_ml::features::{FeatureConfig, FeatureDefinition, NormalizationMethod};
use kairos_ml::model::{Model as MlModel, tch_impl::TchModel};
use kairos_ml::strategy::{MlStrategy, MlStrategyConfig};
use kairos_backtest::Strategy;

/// Thread-safe model wrapper that uses Mutex for interior mutability
struct ThreadSafeModel(Arc<std::sync::Mutex<Box<dyn MlModel>>>);

unsafe impl Send for ThreadSafeModel {}
unsafe impl Sync for ThreadSafeModel {}

impl MlModel for ThreadSafeModel {
    fn predict(&self, input: &kairos_ml::model::Tensor) -> Result<kairos_ml::model::ModelOutput, kairos_ml::model::ModelError> {
        self.0.lock().unwrap().predict(input)
    }
    fn input_shape(&self) -> Vec<i64> {
        self.0.lock().unwrap().input_shape()
    }
    fn output_shape(&self) -> Vec<i64> {
        self.0.lock().unwrap().output_shape()
    }
    fn name(&self) -> &str {
        // Return static string to avoid lifetime issues
        "ml_model"
    }
}

/// Create an ML strategy for backtesting
async fn create_ml_strategy(args: &BacktestArgs, _ticker: &FuturesTicker) -> Result<Box<dyn kairos_backtest::Strategy>> {
    // Get model path
    let model_path = args.model_path.as_ref()
        .with_context(|| "--model-path is required for ml strategy")?;

    if !model_path.exists() {
        anyhow::bail!("Model file not found: {}", model_path.display());
    }

    println!("ML Strategy Configuration");
    println!("========================");
    println!("Model: {}", model_path.display());

    // Create feature configuration matching training
    let feature_config = create_default_feature_config();
    
    println!("Features: {} indicators", feature_config.features.len());
    println!("  - SMA (20, 50)");
    println!("  - EMA (12, 26)");
    println!("  - RSI (14)");
    println!("  - ATR (14)");
    println!("  - MACD (12, 26, 9)");
    println!("  - Bollinger Bands (20, 2)");
    println!("  - VWAP");
    println!("Lookback: {} bars", feature_config.lookback_periods);
    println!();

    // Load strategy config from file if provided
    let strategy_config = if let Some(config_path) = &args.strategy_config {
        let content = std::fs::read_to_string(config_path)?;
        serde_json::from_str::<MlStrategyConfig>(&content)?
            .with_feature_config(feature_config.clone())
    } else {
        MlStrategyConfig::new(feature_config.clone())
    };

    // Create ML strategy
    let mut strategy = MlStrategy::new(strategy_config);

    // Load the trained model
    println!("Loading trained model...");
    match TchModel::load(model_path) {
        Ok(model) => {
            println!("Model loaded successfully");
            println!("  Name: {}", model.name());
            let wrapped = ThreadSafeModel(Arc::new(std::sync::Mutex::new(Box::new(model))));
            strategy.set_model(Arc::new(wrapped) as Arc<dyn MlModel + Send + Sync>);
        }
        Err(e) => {
            eprintln!("Warning: Failed to load model weights: {}", e);
            eprintln!("Creating default model (training required for good results)");
            let default_model = TchModel::new(240, 64, 3, "default_model");
            let wrapped = ThreadSafeModel(Arc::new(std::sync::Mutex::new(Box::new(default_model))));
            strategy.set_model(Arc::new(wrapped) as Arc<dyn MlModel + Send + Sync>);
        }
    }

    println!("\nML Strategy initialized successfully!");
    println!("Required studies:");

    let required_studies = strategy.required_studies();
    for study in &required_studies {
        println!("  - {} (study: {})", study.key, study.study_id);
    }

    Ok(Box::new(strategy))
}

/// Create default feature configuration matching the training setup
fn create_default_feature_config() -> FeatureConfig {
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
            // MACD - composite output with Lines + Histogram
            // For MACD, "line" returns the first line (MACD main)
            // For MACD Signal, we use numeric index "lines.1" (second line is Signal)
            // For MACD Histogram, we use "histogram"
            FeatureDefinition::new("macd", "lines.0"),
            FeatureDefinition::new("macd_signal", "lines.1"),
            FeatureDefinition::new("macd_hist", "histogram"),
            // Bollinger Bands (study ID is "bollinger", not "bb")
            FeatureDefinition::new("bollinger_upper", "band.upper"),
            FeatureDefinition::new("bollinger_lower", "band.lower"),
            // Volume
            FeatureDefinition::new("vwap", "value"),
        ],
        lookback_periods: 20,
        normalization: NormalizationMethod::None,
    }
}
