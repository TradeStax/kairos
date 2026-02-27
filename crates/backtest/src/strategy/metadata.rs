//! Strategy metadata and categorization.
//!
//! [`StrategyMetadata`] provides descriptive information about a
//! strategy for display in the UI and serialization.
//! [`StrategyCategory`] groups strategies into high-level families.

use serde::{Deserialize, Serialize};

/// Descriptive metadata for a backtest strategy.
///
/// Returned by [`Strategy::metadata`](super::Strategy::metadata) and
/// used by the UI to display strategy names, descriptions, and
/// categories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyMetadata {
    /// Unique strategy identifier (matches
    /// [`Strategy::id`](super::Strategy::id)).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Brief description of the strategy's approach.
    pub description: String,
    /// High-level category for grouping in the UI.
    pub category: StrategyCategory,
    /// Semantic version string (e.g. `"1.0.0"`).
    pub version: &'static str,
}

/// High-level category for grouping strategies in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StrategyCategory {
    /// Breakout and momentum-based strategies.
    BreakoutMomentum,
    /// Mean-reversion and fade strategies.
    MeanReversion,
    /// Trend-following strategies.
    TrendFollowing,
    /// Order-flow-based strategies.
    OrderFlow,
    /// User-defined strategies.
    Custom,
}

impl std::fmt::Display for StrategyCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BreakoutMomentum => {
                write!(f, "Breakout / Momentum")
            }
            Self::MeanReversion => write!(f, "Mean Reversion"),
            Self::TrendFollowing => write!(f, "Trend Following"),
            Self::OrderFlow => write!(f, "Order Flow"),
            Self::Custom => write!(f, "Custom"),
        }
    }
}
