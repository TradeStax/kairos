use crate::output::metrics::PerformanceMetrics;
use serde::{Deserialize, Serialize};

/// Objective function for optimization.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ObjectiveFunction {
    NetPnl,
    SharpeRatio,
    SortinoRatio,
    ProfitFactor,
    CalmarRatio,
    WinRate,
    Expectancy,
}

impl ObjectiveFunction {
    /// Extract the objective value from metrics.
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
            Self::SharpeRatio => {
                write!(f, "Sharpe Ratio")
            }
            Self::SortinoRatio => {
                write!(f, "Sortino Ratio")
            }
            Self::ProfitFactor => {
                write!(f, "Profit Factor")
            }
            Self::CalmarRatio => {
                write!(f, "Calmar Ratio")
            }
            Self::WinRate => write!(f, "Win Rate"),
            Self::Expectancy => {
                write!(f, "Expectancy")
            }
        }
    }
}
