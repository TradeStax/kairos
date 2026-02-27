//! Complete backtest result container.
//!
//! [`BacktestResult`] bundles every artifact produced by a finished
//! backtest run: the configuration that was used, all completed
//! trades, aggregated performance metrics, the equity curve, and
//! diagnostic metadata such as warnings and session counts.

use crate::config::backtest::BacktestConfig;
use crate::output::metrics::PerformanceMetrics;
use crate::output::trade_record::TradeRecord;
use crate::portfolio::equity::EquityCurve;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Complete output of a finished backtest run.
///
/// Produced by [`BacktestRunner`](crate::engine::runner::BacktestRunner)
/// after processing all sessions. Contains everything needed to
/// display analytics, persist results, or compare across runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    /// Unique identifier for this run.
    pub id: Uuid,
    /// The configuration that was used for this run.
    pub config: BacktestConfig,
    /// Strategy display name (sourced from strategy metadata).
    pub strategy_name: String,
    /// Unix millisecond timestamp when the run started.
    pub run_started_at_ms: u64,
    /// Wall-clock duration of the run, in milliseconds.
    pub run_duration_ms: u64,
    /// Total number of raw trade ticks processed across all
    /// sessions.
    pub total_data_trades: usize,
    /// All completed round-trip trades in chronological order.
    pub trades: Vec<TradeRecord>,
    /// Aggregated performance statistics computed from `trades`.
    pub metrics: PerformanceMetrics,
    /// Equity curve sampled throughout the run.
    pub equity_curve: EquityCurve,
    /// Number of RTH sessions that were fully processed.
    pub sessions_processed: usize,
    /// Number of RTH sessions skipped (e.g. holidays with no
    /// data).
    pub sessions_skipped: usize,
    /// Non-fatal warnings generated during the run (e.g. data
    /// gaps, rejected orders).
    pub warnings: Vec<String>,
    /// Daily equity snapshots for post-run analytics and charting.
    #[serde(default)]
    pub daily_snapshots: Vec<crate::portfolio::equity::DailySnapshot>,
    /// Buy-and-hold benchmark P&L in USD, if computed.
    #[serde(default)]
    pub benchmark_pnl_usd: Option<f64>,
}
