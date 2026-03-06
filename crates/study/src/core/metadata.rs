//! Study metadata, classification enums, and capability flags.
//!
//! [`StudyMetadata`] consolidates all study metadata into a single struct.
//! [`StudyCapabilities`] declares which optional features a study supports.
//! [`StudyCategory`] groups studies for menu and search UI.
//! [`StudyPlacement`] determines where a study renders relative to the
//! price chart (overlay, separate panel, background, etc.).

use serde::{Deserialize, Serialize};

/// Consolidated metadata for a study instance.
///
/// Replaces the individual `name()`, `category()`, `placement()` methods
/// with a single struct returned by [`Study::metadata()`](super::Study::metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyMetadata {
    /// Human-readable display name (e.g. "Simple Moving Average").
    pub name: String,
    /// Functional category for grouping in the catalog.
    pub category: StudyCategory,
    /// Where this study renders relative to the price chart.
    pub placement: StudyPlacement,
    /// Short description for tooltips and help text.
    pub description: String,
    /// Schema version for parameter persistence migration.
    pub config_version: u16,
    /// Optional feature flags.
    pub capabilities: StudyCapabilities,
}

/// Declares which optional features a study supports.
///
/// Used by the UI layer to conditionally enable interactive features,
/// by the chart engine for rendering optimization, and by the registry
/// for catalog filtering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StudyCapabilities {
    /// Supports incremental `append_trades()` optimization.
    pub incremental: bool,
    /// Provides interactive data for UI modals and overlays.
    pub interactive: bool,
    /// Recomputes when the visible range changes (pan/zoom).
    pub needs_visible_range: bool,
    /// Requires raw trade data (not just candles).
    pub needs_trades: bool,
    /// Has a detail modal accessible from the chart overlay.
    pub has_detail_modal: bool,
    /// Accepts externally-provided data (e.g. manual levels).
    pub accepts_external_data: bool,
    /// Replaces the standard candle rendering.
    pub candle_replace: bool,
    /// Uses custom rendering via [`DrawContext`](super::draw_context::DrawContext).
    pub custom_draw: bool,
}

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
