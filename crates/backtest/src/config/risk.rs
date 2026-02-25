use serde::{Deserialize, Serialize};

/// Risk and position-sizing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// How to calculate the number of contracts per trade.
    pub position_size_mode: PositionSizeMode,
    /// Maximum number of concurrently open positions. Default: 1.
    pub max_concurrent_positions: usize,
    /// Stop the run if equity drawdown exceeds this fraction of initial capital.
    /// E.g. 0.20 = stop when drawdown reaches 20%. `None` means no limit.
    pub max_drawdown_pct: Option<f64>,
    /// Annual risk-free rate used for Sharpe and Sortino computation (e.g. 0.05 = 5%).
    pub risk_free_annual: f64,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            position_size_mode: PositionSizeMode::Fixed(1.0),
            max_concurrent_positions: 1,
            max_drawdown_pct: None,
            risk_free_annual: 0.05,
        }
    }
}

/// Determines how many contracts are bought/sold on entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionSizeMode {
    /// Always trade exactly N contracts.
    Fixed(f64),
    /// Risk a fraction of current equity per trade.
    /// contracts = (equity * pct) / stop_distance_usd.
    RiskPercent(f64),
    /// Risk a fixed USD amount per trade.
    /// contracts = risk_usd / stop_distance_usd.
    RiskDollars(f64),
}

/// Model for simulating fill price slippage.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum SlippageModel {
    /// No slippage — fills at exact price.
    #[default]
    None,
    /// Adverse N-tick slippage on every fill (entry and exit).
    FixedTick(i64),
    /// Percentage of fill price adverse slippage (e.g. 0.0001 = 1 basis point).
    Percentage(f64),
}
