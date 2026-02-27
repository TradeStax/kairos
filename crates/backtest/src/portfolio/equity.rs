//! Equity curve and daily snapshot tracking.
//!
//! Records the portfolio's equity over time at two granularities:
//!
//! - **Tick-level** via [`EquityCurve`] -- every sample captured by
//!   the engine (typically per-bar or per-fill).
//! - **Daily** via [`DailyEquityTracker`] -- one snapshot per
//!   calendar day, useful for computing daily returns and Sharpe.

use kairos_data::Timestamp;
use serde::{Deserialize, Serialize};

/// A single sample point on the equity curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    /// Timestamp of this sample (milliseconds since epoch).
    pub timestamp: Timestamp,
    /// Realized equity from closed trades only (cash balance).
    pub realized_equity_usd: f64,
    /// Unrealized PnL from open positions at mark-to-market.
    pub unrealized_pnl_usd: f64,
    /// Total equity = realized + unrealized.
    pub total_equity_usd: f64,
}

/// Full equity curve for a backtest run.
///
/// Stores an ordered sequence of [`EquityPoint`] samples that
/// represent the portfolio's value over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityCurve {
    /// Time-ordered equity samples.
    pub points: Vec<EquityPoint>,
    /// Starting equity for this backtest run (USD).
    pub initial_equity_usd: f64,
}

impl EquityCurve {
    /// Create a new empty equity curve with the given starting
    /// equity.
    #[must_use]
    pub fn new(initial_equity_usd: f64) -> Self {
        Self {
            points: Vec::new(),
            initial_equity_usd,
        }
    }

    /// Record a new equity sample.
    ///
    /// `realized` is the cash balance (initial equity + closed-trade
    /// PnL - commissions). `unrealized` is the mark-to-market PnL of
    /// open positions.
    pub fn record(&mut self, timestamp: Timestamp, realized: f64, unrealized: f64) {
        self.points.push(EquityPoint {
            timestamp,
            realized_equity_usd: realized,
            unrealized_pnl_usd: unrealized,
            total_equity_usd: realized + unrealized,
        });
    }

    /// Current realized equity (last recorded point, or initial if
    /// no samples yet).
    #[must_use]
    pub fn current_realized(&self) -> f64 {
        self.points
            .last()
            .map(|p| p.realized_equity_usd)
            .unwrap_or(self.initial_equity_usd)
    }
}

/// Daily equity snapshot for end-of-day tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailySnapshot {
    /// Start-of-day timestamp (milliseconds, midnight UTC).
    pub date_ms: u64,
    /// Total equity at the time of the snapshot.
    pub equity: f64,
    /// Cumulative realized PnL at end of day.
    pub realized_pnl: f64,
    /// Number of open positions at end of day.
    pub positions_count: usize,
}

/// Tracks daily equity snapshots for analytics.
///
/// Records at most one snapshot per calendar day (UTC). The day
/// boundary is determined by integer-dividing the timestamp by
/// 86,400,000 ms.
pub struct DailyEquityTracker {
    /// Collected daily snapshots in chronological order.
    snapshots: Vec<DailySnapshot>,
    /// Day index of the most recent snapshot (timestamp_ms / 86.4M).
    last_day: Option<u64>,
}

impl DailyEquityTracker {
    /// Create a new empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            last_day: None,
        }
    }

    /// Record a snapshot if a new calendar day (UTC) has started.
    ///
    /// The day boundary is `timestamp_ms / 86_400_000`. If the
    /// current timestamp falls on the same day as the last recorded
    /// snapshot, this is a no-op.
    pub fn maybe_record(
        &mut self,
        timestamp_ms: u64,
        equity: f64,
        realized_pnl: f64,
        positions_count: usize,
    ) {
        let day = timestamp_ms / 86_400_000;
        if self.last_day != Some(day) {
            self.last_day = Some(day);
            self.snapshots.push(DailySnapshot {
                date_ms: day * 86_400_000,
                equity,
                realized_pnl,
                positions_count,
            });
        }
    }

    /// All recorded daily snapshots in chronological order.
    #[must_use]
    pub fn snapshots(&self) -> &[DailySnapshot] {
        &self.snapshots
    }

    /// Clear all snapshots and reset the day tracker.
    pub fn reset(&mut self) {
        self.snapshots.clear();
        self.last_day = None;
    }
}

impl Default for DailyEquityTracker {
    fn default() -> Self {
        Self::new()
    }
}
