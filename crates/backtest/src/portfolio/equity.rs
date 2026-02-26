use kairos_data::Timestamp;
use serde::{Deserialize, Serialize};

/// A single sample point on the equity curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp: Timestamp,
    /// Realized equity (closed trades only).
    pub realized_equity_usd: f64,
    /// Unrealized PnL from any open position.
    pub unrealized_pnl_usd: f64,
    /// Total equity = realized + unrealized.
    pub total_equity_usd: f64,
}

/// Full equity curve for a backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityCurve {
    pub points: Vec<EquityPoint>,
    pub initial_equity_usd: f64,
}

impl EquityCurve {
    pub fn new(initial_equity_usd: f64) -> Self {
        Self {
            points: Vec::new(),
            initial_equity_usd,
        }
    }

    /// Record a new equity sample.
    pub fn record(&mut self, timestamp: Timestamp, realized: f64, unrealized: f64) {
        self.points.push(EquityPoint {
            timestamp,
            realized_equity_usd: realized,
            unrealized_pnl_usd: unrealized,
            total_equity_usd: realized + unrealized,
        });
    }

    /// Current realized equity (last recorded point, or initial if empty).
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
    pub date_ms: u64,
    pub equity: f64,
    pub realized_pnl: f64,
    pub positions_count: usize,
}

/// Tracks daily equity snapshots for analytics.
pub struct DailyEquityTracker {
    snapshots: Vec<DailySnapshot>,
    last_day: Option<u64>,
}

impl DailyEquityTracker {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            last_day: None,
        }
    }

    /// Record a snapshot if a new day has started.
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

    pub fn snapshots(&self) -> &[DailySnapshot] {
        &self.snapshots
    }

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
