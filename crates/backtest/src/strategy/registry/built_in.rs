//! Registration of built-in strategies.
//!
//! Called by [`StrategyRegistry::with_built_ins`](super::StrategyRegistry::with_built_ins)
//! to populate the registry with the default strategy set.

use crate::strategy::built_in::momentum_breakout::MomentumBreakoutStrategy;
use crate::strategy::built_in::orb::OrbStrategy;
use crate::strategy::built_in::vwap_reversion::VwapReversionStrategy;
use crate::strategy::metadata::StrategyCategory;
use crate::strategy::registry::{StrategyInfo, StrategyRegistry};

/// Registers all built-in strategies with the given registry.
pub fn register_all(registry: &mut StrategyRegistry) {
    registry.register(
        "orb",
        StrategyInfo {
            id: "orb".to_string(),
            name: "Opening Range Breakout".to_string(),
            description: "Trades breakouts above/below the \
                          first N minutes of the RTH session."
                .to_string(),
            category: StrategyCategory::BreakoutMomentum,
        },
        || Box::new(OrbStrategy::new()),
    );

    registry.register(
        "vwap_reversion",
        StrategyInfo {
            id: "vwap_reversion".to_string(),
            name: "VWAP Reversion".to_string(),
            description: "Fades price deviations from VWAP \
                          at standard-deviation bands."
                .to_string(),
            category: StrategyCategory::MeanReversion,
        },
        || Box::new(VwapReversionStrategy::new()),
    );

    registry.register(
        "momentum_breakout",
        StrategyInfo {
            id: "momentum_breakout".to_string(),
            name: "Momentum Breakout".to_string(),
            description: "Donchian channel breakout with \
                          ATR-scaled bracket orders."
                .to_string(),
            category: StrategyCategory::TrendFollowing,
        },
        || Box::new(MomentumBreakoutStrategy::new()),
    );
}
