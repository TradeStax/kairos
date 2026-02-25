use crate::core::metadata::StrategyCategory;
use crate::registry::{StrategyInfo, StrategyRegistry};
use crate::strategies::momentum_breakout::MomentumBreakoutStrategy;
use crate::strategies::orb::OrbStrategy;
use crate::strategies::vwap_reversion::VwapReversionStrategy;

pub fn register_all(registry: &mut StrategyRegistry) {
    registry.register(
        "orb",
        StrategyInfo {
            id: "orb".to_string(),
            name: "Opening Range Breakout".to_string(),
            description:
                "Trades breakouts above/below the first N minutes of the RTH session."
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
            description:
                "Fades price deviations from VWAP at standard-deviation bands.".to_string(),
            category: StrategyCategory::MeanReversion,
        },
        || Box::new(VwapReversionStrategy::new()),
    );

    registry.register(
        "momentum_breakout",
        StrategyInfo {
            id: "momentum_breakout".to_string(),
            name: "Momentum Breakout".to_string(),
            description:
                "Donchian channel breakout with ATR-scaled bracket orders.".to_string(),
            category: StrategyCategory::TrendFollowing,
        },
        || Box::new(MomentumBreakoutStrategy::new()),
    );
}
