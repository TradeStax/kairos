use super::{StudyInfo, StudyRegistry};
use crate::core::{StudyCategory, StudyPlacement};

pub(super) fn register_built_ins(registry: &mut StudyRegistry) {
    // Volume studies
    registry.register(
        "volume",
        StudyInfo {
            id: "volume".to_string(),
            name: "Volume".to_string(),
            category: StudyCategory::Volume,
            placement: StudyPlacement::Panel,
            description: "Total volume per candle".to_string(),
        },
        || Box::new(crate::studies::volume::VolumeStudy::new()),
    );

    registry.register(
        "delta",
        StudyInfo {
            id: "delta".to_string(),
            name: "Volume Delta".to_string(),
            category: StudyCategory::Volume,
            placement: StudyPlacement::Panel,
            description: "Buy minus sell volume per candle".to_string(),
        },
        || Box::new(crate::studies::volume::DeltaStudy::new()),
    );

    // Order flow studies
    registry.register(
        "imbalance",
        StudyInfo {
            id: "imbalance".to_string(),
            name: "Imbalance".to_string(),
            category: StudyCategory::OrderFlow,
            placement: StudyPlacement::Background,
            description: "Price levels with significant buy/sell imbalance".to_string(),
        },
        || Box::new(crate::studies::orderflow::ImbalanceStudy::new()),
    );

    registry.register(
        "big_trades",
        StudyInfo {
            id: "big_trades".to_string(),
            name: "Big Trades".to_string(),
            category: StudyCategory::OrderFlow,
            placement: StudyPlacement::Overlay,
            description: "Aggregated institutional-scale trade bubbles".to_string(),
        },
        || Box::new(crate::studies::orderflow::BigTradesStudy::new()),
    );

    registry.register(
        "footprint",
        StudyInfo {
            id: "footprint".to_string(),
            name: "Footprint".to_string(),
            category: StudyCategory::OrderFlow,
            placement: StudyPlacement::CandleReplace,
            description: "Per-candle trade volume at each price level".to_string(),
        },
        || Box::new(crate::studies::orderflow::FootprintStudy::new()),
    );

    registry.register(
        "vbp",
        StudyInfo {
            id: "vbp".to_string(),
            name: "Volume by Price".to_string(),
            category: StudyCategory::OrderFlow,
            placement: StudyPlacement::Background,
            description: "Horizontal volume distribution bars at each price level".to_string(),
        },
        || Box::new(crate::studies::orderflow::VbpStudy::new()),
    );

    // Trend studies
    registry.register(
        "sma",
        StudyInfo {
            id: "sma".to_string(),
            name: "Simple Moving Average".to_string(),
            category: StudyCategory::Trend,
            placement: StudyPlacement::Overlay,
            description: "Simple moving average of price".to_string(),
        },
        || Box::new(crate::studies::trend::sma::SmaStudy::new()),
    );

    registry.register(
        "ema",
        StudyInfo {
            id: "ema".to_string(),
            name: "Exponential Moving Average".to_string(),
            category: StudyCategory::Trend,
            placement: StudyPlacement::Overlay,
            description: "Exponential moving average of price".to_string(),
        },
        || Box::new(crate::studies::trend::ema::EmaStudy::new()),
    );

    registry.register(
        "vwap",
        StudyInfo {
            id: "vwap".to_string(),
            name: "Volume Weighted Average Price".to_string(),
            category: StudyCategory::Trend,
            placement: StudyPlacement::Overlay,
            description: "Volume weighted average price with optional bands".to_string(),
        },
        || Box::new(crate::studies::trend::vwap::VwapStudy::new()),
    );

    // Volume studies (continued)
    registry.register(
        "cvd",
        StudyInfo {
            id: "cvd".to_string(),
            name: "Cumulative Volume Delta".to_string(),
            category: StudyCategory::Volume,
            placement: StudyPlacement::Panel,
            description: "Cumulative sum of buy minus sell volume".to_string(),
        },
        || Box::new(crate::studies::volume::CvdStudy::new()),
    );

    registry.register(
        "obv",
        StudyInfo {
            id: "obv".to_string(),
            name: "On Balance Volume".to_string(),
            category: StudyCategory::Volume,
            placement: StudyPlacement::Panel,
            description: "Cumulative volume based on price direction".to_string(),
        },
        || Box::new(crate::studies::volume::ObvStudy::new()),
    );

    // Volatility studies
    registry.register(
        "atr",
        StudyInfo {
            id: "atr".to_string(),
            name: "Average True Range".to_string(),
            category: StudyCategory::Volatility,
            placement: StudyPlacement::Panel,
            description: "Average true range using Wilder's smoothing".to_string(),
        },
        || Box::new(crate::studies::volatility::atr::AtrStudy::new()),
    );

    registry.register(
        "bollinger",
        StudyInfo {
            id: "bollinger".to_string(),
            name: "Bollinger Bands".to_string(),
            category: StudyCategory::Volatility,
            placement: StudyPlacement::Overlay,
            description: "SMA with standard deviation bands".to_string(),
        },
        || Box::new(crate::studies::volatility::bollinger::BollingerStudy::new()),
    );

    // Momentum studies
    registry.register(
        "rsi",
        StudyInfo {
            id: "rsi".to_string(),
            name: "Relative Strength Index".to_string(),
            category: StudyCategory::Momentum,
            placement: StudyPlacement::Panel,
            description: "Momentum oscillator measuring overbought/oversold conditions".to_string(),
        },
        || Box::new(crate::studies::momentum::rsi::RsiStudy::new()),
    );

    registry.register(
        "macd",
        StudyInfo {
            id: "macd".to_string(),
            name: "MACD".to_string(),
            category: StudyCategory::Momentum,
            placement: StudyPlacement::Panel,
            description: "Moving Average Convergence Divergence".to_string(),
        },
        || Box::new(crate::studies::momentum::macd::MacdStudy::new()),
    );

    registry.register(
        "stochastic",
        StudyInfo {
            id: "stochastic".to_string(),
            name: "Stochastic Oscillator".to_string(),
            category: StudyCategory::Momentum,
            placement: StudyPlacement::Panel,
            description: "Stochastic oscillator with %K and %D lines".to_string(),
        },
        || Box::new(crate::studies::momentum::stochastic::StochasticStudy::new()),
    );
}
