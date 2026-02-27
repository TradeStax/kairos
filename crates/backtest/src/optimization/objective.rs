//! Objective functions used to score parameter combinations during
//! optimization.
//!
//! Each variant maps to a single scalar extracted from
//! [`PerformanceMetrics`], allowing the optimizer to rank parameter
//! sets by whichever criterion matters most for the strategy.

use crate::output::metrics::PerformanceMetrics;
use serde::{Deserialize, Serialize};

/// Objective function for optimization scoring.
///
/// Determines which performance metric the optimizer maximizes when
/// searching the parameter grid. Higher values are always better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjectiveFunction {
    /// Maximize total net profit/loss in USD.
    NetPnl,
    /// Maximize annualized Sharpe ratio (risk-adjusted return).
    SharpeRatio,
    /// Maximize annualized Sortino ratio (downside-risk-adjusted).
    SortinoRatio,
    /// Maximize profit factor (gross wins / gross losses).
    ProfitFactor,
    /// Maximize Calmar ratio (annualized return / max drawdown).
    CalmarRatio,
    /// Maximize win rate (fraction of profitable trades).
    WinRate,
    /// Maximize per-trade expectancy in USD.
    Expectancy,
}

impl ObjectiveFunction {
    /// Extracts the scalar objective value from the given metrics.
    ///
    /// For [`ProfitFactor`](Self::ProfitFactor), infinite values
    /// (which occur when there are no losing trades) are capped at
    /// 1000.0 to keep optimization numerically stable.
    #[must_use]
    pub fn evaluate(&self, metrics: &PerformanceMetrics) -> f64 {
        match self {
            Self::NetPnl => metrics.net_pnl_usd,
            Self::SharpeRatio => metrics.sharpe_ratio,
            Self::SortinoRatio => metrics.sortino_ratio,
            Self::ProfitFactor => {
                if metrics.profit_factor.is_infinite() {
                    1000.0
                } else {
                    metrics.profit_factor
                }
            }
            Self::CalmarRatio => metrics.calmar_ratio,
            Self::WinRate => metrics.win_rate,
            Self::Expectancy => metrics.expectancy_usd,
        }
    }
}

impl std::fmt::Display for ObjectiveFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetPnl => write!(f, "Net PnL"),
            Self::SharpeRatio => write!(f, "Sharpe Ratio"),
            Self::SortinoRatio => write!(f, "Sortino Ratio"),
            Self::ProfitFactor => write!(f, "Profit Factor"),
            Self::CalmarRatio => write!(f, "Calmar Ratio"),
            Self::WinRate => write!(f, "Win Rate"),
            Self::Expectancy => write!(f, "Expectancy"),
        }
    }
}
