use serde::{Deserialize, Serialize};

/// Metadata describing a backtest strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: StrategyCategory,
    pub version: &'static str,
}

/// High-level category for grouping strategies in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StrategyCategory {
    BreakoutMomentum,
    MeanReversion,
    TrendFollowing,
    OrderFlow,
    Custom,
}

impl std::fmt::Display for StrategyCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BreakoutMomentum => write!(f, "Breakout / Momentum"),
            Self::MeanReversion => write!(f, "Mean Reversion"),
            Self::TrendFollowing => write!(f, "Trend Following"),
            Self::OrderFlow => write!(f, "Order Flow"),
            Self::Custom => write!(f, "Custom"),
        }
    }
}
