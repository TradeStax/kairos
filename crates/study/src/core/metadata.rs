//! Study classification enums: category and placement.
//!
//! [`StudyCategory`] groups studies for menu and search UI.
//! [`StudyPlacement`] determines where a study renders relative to the
//! price chart (overlay, separate panel, background, etc.).

/// Study category for grouping in menus and search.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum StudyCategory {
    /// Moving averages and trend-following indicators (SMA, EMA, VWAP).
    Trend,
    /// Oscillators measuring speed of price movement (RSI, MACD, Stochastic).
    Momentum,
    /// Volume-derived indicators (Volume bars, OBV, CVD).
    Volume,
    /// Measures of price dispersion (ATR, Bollinger Bands).
    Volatility,
    /// Depth-of-market and trade-level analytics (Volume Profile, Imbalance).
    OrderFlow,
    /// User-defined or uncategorized studies.
    #[default]
    Custom,
}

impl std::fmt::Display for StudyCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StudyCategory::Trend => write!(f, "Trend"),
            StudyCategory::Momentum => write!(f, "Momentum"),
            StudyCategory::Volume => write!(f, "Volume"),
            StudyCategory::Volatility => write!(f, "Volatility"),
            StudyCategory::OrderFlow => write!(f, "Order Flow"),
            StudyCategory::Custom => write!(f, "Custom"),
        }
    }
}

/// Where a study renders relative to the price chart.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum StudyPlacement {
    /// Drawn on the price chart (SMA, Bollinger, VWAP).
    #[default]
    Overlay,
    /// Separate panel below chart (RSI, MACD, Volume).
    Panel,
    /// Behind candles (Volume Profile, Value Area).
    Background,
    /// Replaces standard candle rendering entirely.
    /// Only one `CandleReplace` study can be active at a time.
    CandleReplace,
    /// Dedicated side panel to the right of the chart, sharing the Y (price) axis.
    SidePanel,
}

impl std::fmt::Display for StudyPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StudyPlacement::Overlay => write!(f, "Overlay"),
            StudyPlacement::Panel => write!(f, "Panel"),
            StudyPlacement::Background => write!(f, "Background"),
            StudyPlacement::CandleReplace => write!(f, "Candle Replace"),
            StudyPlacement::SidePanel => write!(f, "Side Panel"),
        }
    }
}
