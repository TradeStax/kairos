use crate::config::backtest::BacktestConfig;
use crate::output::metrics::PerformanceMetrics;
use crate::output::trade_record::TradeRecord;
use crate::portfolio::equity::EquityCurve;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Complete output of a finished backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    /// Unique run identifier.
    pub id: Uuid,
    /// The configuration used for this run.
    pub config: BacktestConfig,
    /// Strategy display name (from metadata).
    pub strategy_name: String,
    /// Unix ms when the run started.
    pub run_started_at_ms: u64,
    /// Wall-clock duration of the run in ms.
    pub run_duration_ms: u64,
    /// Total number of raw trade ticks processed.
    pub total_data_trades: usize,
    /// All completed trades in chronological order.
    pub trades: Vec<TradeRecord>,
    /// Aggregated performance statistics.
    pub metrics: PerformanceMetrics,
    /// Equity curve sampled throughout the run.
    pub equity_curve: EquityCurve,
    /// Number of RTH sessions fully processed.
    pub sessions_processed: usize,
    /// Number of RTH sessions skipped (e.g. holidays with no data).
    pub sessions_skipped: usize,
    /// Non-fatal warnings generated during the run.
    pub warnings: Vec<String>,
    /// Daily equity snapshots for analytics.
    #[serde(default)]
    pub daily_snapshots: Vec<crate::portfolio::equity::DailySnapshot>,
    /// Buy-and-hold benchmark PnL for comparison.
    #[serde(default)]
    pub benchmark_pnl_usd: Option<f64>,
}
