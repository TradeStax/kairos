//! # kairos-backtest
//!
//! Event-driven backtesting engine for futures trading strategies.
//!
//! This crate provides a deterministic simulation framework that replays
//! historical trade data through user-defined strategies, tracking
//! orders, fills, positions, and portfolio equity with tick-level
//! fidelity.
//!
//! # Architecture
//!
//! ```text
//! BacktestRunner
//!     │
//!     ▼
//! TradeProvider ──> DataFeed ──> Engine
//!                                 │
//!                    ┌────────────┼────────────┐
//!                    ▼            ▼             ▼
//!               Strategy    FillSimulator   Portfolio
//!                    │            │             │
//!                    └──> OrderBook <───────────┘
//!                              │
//!                              ▼
//!                       BacktestResult
//! ```
//!
//! # Modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`config`] | Backtest configuration, risk rules, margin, slippage |
//! | [`engine`] | Core simulation loop and high-level runner facade |
//! | [`feed`] | Historical data ingestion and candle aggregation |
//! | [`fill`] | Fill simulation (standard slippage, depth-based) |
//! | [`order`] | Order lifecycle: creation, matching, cancellation |
//! | [`portfolio`] | Position tracking, equity curve, margin enforcement |
//! | [`strategy`] | Strategy trait, built-in strategies, registry |
//! | [`output`] | Result types, performance metrics, progress events |
//! | [`analysis`] | Statistical analysis, Monte Carlo simulation |
//! | [`optimization`] | Walk-forward optimization, parameter grid search |
//!
//! # Typical usage
//!
//! ```ignore
//! use kairos_backtest::prelude::*;
//!
//! let provider: Arc<dyn TradeProvider> = /* ... */;
//! let runner = BacktestRunner::new(provider);
//!
//! let registry = StrategyRegistry::new();
//! let strategy = registry.create("orb", &BacktestConfig::default())?;
//!
//! let result = runner.run(config, strategy).await?;
//! println!("Sharpe: {:.2}", result.metrics.sharpe_ratio);
//! ```

pub mod analysis;
pub mod config;
pub mod engine;
pub mod feed;
pub mod fill;
pub mod optimization;
pub mod order;
pub mod output;
pub mod portfolio;
pub mod prelude;
pub mod strategy;

// ─── Configuration ──────────────────────────────────────────────

pub use config::backtest::BacktestConfig;
pub use config::risk::{PositionSizeMode, RiskConfig, SlippageModel};

// ─── Engine ─────────────────────────────────────────────────────

pub use engine::runner::BacktestRunner;

// ─── Data feed ──────────────────────────────────────────────────

pub use feed::provider::TradeProvider;

// ─── Output ─────────────────────────────────────────────────────

pub use output::metrics::PerformanceMetrics;
pub use output::progress::BacktestProgressEvent;
pub use output::result::BacktestResult;
pub use output::trade_record::{ExitReason, TradeRecord};

// ─── Portfolio ──────────────────────────────────────────────────

pub use portfolio::equity::{EquityCurve, EquityPoint};

// ─── Strategy ───────────────────────────────────────────────────

pub use strategy::metadata::{StrategyCategory, StrategyMetadata};
pub use strategy::registry::{StrategyInfo, StrategyRegistry};
pub use strategy::{BacktestError, OrderEvent, Strategy, StrategyContext, StudyBank, StudyRequest};
