//! Risk management and position-sizing configuration.
//!
//! This module defines how the backtest engine sizes positions
//! ([`PositionSizeMode`]), when to halt a run
//! ([`RiskConfig::max_drawdown_pct`]), and how to model fill
//! slippage ([`SlippageModel`]).

use serde::{Deserialize, Serialize};

/// Risk and position-sizing configuration for a backtest run.
///
/// Controls how many contracts are traded per signal, how many
/// positions may be open simultaneously, when to stop the run
/// due to excessive drawdown, and the risk-free rate used for
/// Sharpe/Sortino calculations.
///
/// # Defaults
///
/// - **Position sizing**: fixed 1 contract
/// - **Max concurrent positions**: 1
/// - **Max drawdown**: unlimited (no circuit breaker)
/// - **Risk-free rate**: 5% annual
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskConfig {
    /// How to calculate the number of contracts per trade.
    pub position_size_mode: PositionSizeMode,
    /// Maximum number of concurrently open positions.
    ///
    /// The engine rejects new entries when this limit is reached.
    /// Default: `1`.
    pub max_concurrent_positions: usize,
    /// Stop the run if peak-to-trough equity drawdown exceeds this
    /// fraction of initial capital.
    ///
    /// For example, `Some(0.20)` halts the backtest when drawdown
    /// reaches 20%. `None` means no drawdown limit.
    pub max_drawdown_pct: Option<f64>,
    /// Annual risk-free rate for Sharpe and Sortino computation.
    ///
    /// For example, `0.05` represents a 5% annual rate.
    pub risk_free_annual: f64,
}

impl Default for RiskConfig {
    /// Default risk config: fixed 1 contract, 1 max position, no
    /// drawdown limit, 5% risk-free rate.
    fn default() -> Self {
        Self {
            position_size_mode: PositionSizeMode::Fixed(1.0),
            max_concurrent_positions: 1,
            max_drawdown_pct: None,
            risk_free_annual: 0.05,
        }
    }
}

/// Determines how many contracts are bought or sold on each entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PositionSizeMode {
    /// Always trade exactly `N` contracts (fractional values are
    /// floored to the nearest integer at fill time).
    Fixed(f64),
    /// Risk a fraction of current equity per trade.
    ///
    /// `contracts = (equity * fraction) / stop_distance_usd`.
    /// Requires the strategy to provide a stop distance.
    RiskPercent(f64),
    /// Risk a fixed USD amount per trade.
    ///
    /// `contracts = risk_usd / stop_distance_usd`.
    /// Requires the strategy to provide a stop distance.
    RiskDollars(f64),
}

/// Model for simulating adverse fill-price slippage.
///
/// Applied to both entry and exit fills during backtest execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SlippageModel {
    /// No slippage -- fills at the exact signal price.
    #[default]
    None,
    /// Fixed adverse slippage of `N` ticks on every fill.
    FixedTick(i64),
    /// Percentage-of-price adverse slippage.
    ///
    /// For example, `0.0001` applies 1 basis point of slippage.
    /// Valid range: `0.0..=0.10`.
    Percentage(f64),
    /// Walk the order-book depth snapshot for realistic fill
    /// pricing.
    ///
    /// Requires [`BacktestConfig::use_depth_data`] to be `true`.
    ///
    /// [`BacktestConfig::use_depth_data`]:
    ///     super::BacktestConfig::use_depth_data
    DepthBased,
    /// Volume-impact model where slippage scales with order size
    /// relative to average daily volume.
    ///
    /// `slippage = base_bps * sqrt(qty / adv_pct)`.
    VolumeImpact {
        /// Base slippage in basis points for a reference-sized
        /// order.
        base_bps: f64,
        /// Average daily volume used to normalize order size.
        average_daily_volume: f64,
    },
}
