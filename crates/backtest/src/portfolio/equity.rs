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
        Self { points: Vec::new(), initial_equity_usd }
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
