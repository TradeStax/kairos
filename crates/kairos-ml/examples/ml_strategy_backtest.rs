//! # ML Strategy Backtest Example
//!
//! This example demonstrates how to use the MlStrategy in a backtest.
//!
//! ## Running the Example
//!
//! ```bash
//! cargo run --example ml_strategy_backtest --release
//! ```
//!
//! ## Prerequisites
//!
//! - A trained model file (`trained_model.pt`) - see `train_simple_model.rs`
//! - Historical price data (DBN files)
//!
//! ## Overview
//!
//! The MlStrategy wraps an ML model and provides a complete trading strategy
//! that can be used with the Kairos backtest engine. The strategy:
//!
//! 1. Extracts features from study outputs (indicators)
//! 2. Runs model inference on the feature matrix
//! 3. Generates trading signals (long/short/neutral)
//! 4. Creates order requests based on signals
//!
//! ## Configuration
//!
//! ```rust
//! use kairos_ml::{MlStrategy, MlStrategyConfig, FeatureConfig, FeatureDefinition};
//!
//! let feature_config = FeatureConfig {
//!     features: vec![
//!         FeatureDefinition::new("sma_20", "line"),
//!         FeatureDefinition::new("rsi_14", "value"),
//!     ],
//!     lookback_periods: 20,
//!     normalization: NormalizationMethod::ZScore,
//! };
//!
//! let ml_config = MlStrategyConfig::new(feature_config)
//!     .model_path("trained_model.pt")
//!     .signal_threshold_long(0.6)
//!     .signal_threshold_short(0.6)
//!     .min_confidence(0.5);
//! ```

use kairos_backtest::strategy::Strategy;
use kairos_ml::features::{FeatureConfig, FeatureDefinition, NormalizationMethod};
use kairos_ml::strategy::{MlStrategy, MlStrategyConfig};

fn main() {
    println!("=================================================");
    println!("  Kairos ML - Strategy Backtest Example");
    println!("=================================================");
    println!();

    // Define feature configuration
    //
    // Features are derived from study outputs. Each feature maps to:
    // - A study key (e.g., "sma_20", "rsi_14")
    // - An output field (e.g., "line", "band.upper")
    //
    // Studies must be registered with the StrategyContext's StudyBank
    // before the strategy can extract features.
    let feature_config = FeatureConfig {
        features: vec![
            // Simple Moving Average feature
            FeatureDefinition::new("sma_20", "line"),
            // Relative Strength Index feature
            FeatureDefinition::new("rsi_14", "value"),
            // MACD feature with percentage change transform
            FeatureDefinition::new("macd", "value")
                .with_transform(kairos_ml::features::FeatureTransform::PctChange),
        ],
        lookback_periods: 20,
        normalization: NormalizationMethod::ZScore,
    };

    println!("Feature Configuration:");
    println!("  Lookback periods: {}", feature_config.lookback_periods);
    println!("  Normalization:   {:?}", feature_config.normalization);
    println!("  Features:");
    for (i, feat) in feature_config.features.iter().enumerate() {
        print!("    {}. {} -> {}", i + 1, feat.study_key, feat.output_field);
        if let Some(transform) = feat.transform {
            print!(" [transform: {:?}]", transform);
        }
        println!();
    }
    println!();

    // Create ML Strategy configuration
    //
    // The strategy configuration controls:
    // - Model loading and inference
    // - Signal generation thresholds
    // - Position sizing based on confidence
    let mut ml_config = MlStrategyConfig::new(feature_config);
    ml_config.id = Some("ml_strategy_example".to_string());
    ml_config.name = Some("Example ML Strategy".to_string());
    ml_config.description = Some("A simple ML strategy using SMA, RSI, and MACD".to_string());
    ml_config.signal_threshold_long = 0.6; // 60% probability for long signal
    ml_config.signal_threshold_short = 0.6; // 60% probability for short signal
    ml_config.min_confidence = 0.5; // Require 50% confidence for orders

    println!("ML Strategy Configuration:");
    println!(
        "  ID:               {}",
        ml_config.id.as_ref().unwrap_or(&"ml_strategy".to_string())
    );
    println!(
        "  Name:             {}",
        ml_config
            .name
            .as_ref()
            .unwrap_or(&"ML Strategy".to_string())
    );
    println!("  Signal thresholds:");
    println!(
        "    Long:           >= {:.0}%",
        ml_config.signal_threshold_long * 100.0
    );
    println!(
        "    Short:          >= {:.0}%",
        ml_config.signal_threshold_short * 100.0
    );
    println!(
        "  Min confidence:    {:.0}%",
        ml_config.min_confidence * 100.0
    );
    println!();

    // Create the strategy
    let strategy = MlStrategy::new(ml_config.clone());

    println!("Strategy created:");
    println!("  ID:               {}", strategy.id());
    println!("  Required studies: {}", strategy.required_studies().len());
    println!();

    // Show how to load a model
    //
    // In a real backtest, the model would be loaded during strategy initialization:
    //
    // ```rust
    // let mut strategy = MlStrategy::new(ml_config);
    // let model = TchModel::load_from_file("trained_model.pt", "production_model")?;
    // strategy.set_model(Arc::new(model));
    // ```
    println!("Model Loading:");
    println!("  To load a trained model:");
    println!();
    println!("  ```rust");
    println!("  use kairos_ml::model::tch_impl::TchModel;");
    println!("  use std::sync::Arc;");
    println!();
    println!("  let model = TchModel::load_from_file(\"trained_model.pt\", \"my_model\")?;");
    println!("  strategy.set_model(Arc::new(model));");
    println!("  ```");
    println!();

    // Show the complete strategy lifecycle
    println!("Strategy Lifecycle:");
    println!("  1. on_init()       - Initialize strategy and load model");
    println!("  2. on_warmup_complete() - Warmup period finished");
    println!("  3. on_candle()     - Process each candle, generate signals");
    println!("  4. on_tick()       - Process tick data (optional)");
    println!("  5. on_session_open() - Handle session open");
    println!("  6. on_session_close() - Handle session close");
    println!("  7. on_order_event()   - Handle order events");
    println!("  8. reset()         - Reset strategy state");
    println!();

    // Demonstrate strategy state
    println!("Strategy State:");
    println!("  Warmup complete:  {}", strategy.warmup_complete());
    println!("  Bars processed:   {}", strategy.bars_processed());
    println!("  Current signal:   {:?}", strategy.current_signal());
    println!("  Current confidence: {:.2}", strategy.current_confidence());
    println!();

    // Show how to run a backtest
    //
    // In a real application, the strategy would be registered with
    // the StrategyRegistry and run as part of a backtest:
    //
    // ```rust
    // use kairos_backtest::engine::BacktestEngine;
    //
    // let mut engine = BacktestEngine::new(config);
    // engine.add_strategy(strategy);
    // let result = engine.run().await?;
    // ```
    println!("Running a Backtest:");
    println!();
    println!("  To run a backtest with this strategy, use:");
    println!();
    println!("  ```bash");
    println!("  kairos backtest \\");
    println!("    --symbol NQ \\");
    println!("    --start 2023-01-01 \\");
    println!("    --end 2023-12-31 \\");
    println!("    --strategy ml_strategy_example \\");
    println!("    --data-dir /path/to/dbn/files \\");
    println!("    --model trained_model.pt");
    println!("  ```");
    println!();

    // Compare to baseline strategies
    println!("Baseline Comparison:");
    println!();
    println!("  A good ML strategy should outperform:");
    println!("  - Random entry (50/50 long/short)");
    println!("  - Buy and hold");
    println!("  - Simple rule-based strategies (e.g., SMA crossover)");
    println!();
    println!("  Key metrics to compare:");
    println!("  - Total return");
    println!("  - Sharpe ratio");
    println!("  - Maximum drawdown");
    println!("  - Win rate");
    println!("  - Profit factor");
    println!();

    println!("=================================================");
    println!("  Example Complete");
    println!("=================================================");
    println!();
    println!("Next steps:");
    println!("  1. Train a model: cargo run --example train_simple_model");
    println!("  2. Run a backtest: kairos backtest --strategy ml_strategy");
    println!("  3. Evaluate results against baseline strategies");
    println!();
}
