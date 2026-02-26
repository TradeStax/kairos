//! Abstract render primitives produced by studies.
//!
//! The chart rendering layer converts these into canvas draw calls. Each study
//! returns a [`StudyOutput`] variant matching its visualization type.
//!
//! - [`series`] — Line, bar, histogram, and price level primitives.
//! - [`markers`] — Trade marker bubbles (Big Trades study).
//! - [`footprint`] — Per-candle trade-level data and configuration enums.
//! - [`profile`] — Volume profile output and VBP sub-feature configs.

pub mod footprint;
mod markers;
pub mod profile;
mod series;

pub use footprint::{
    BackgroundColorMode, CandleRenderConfig, FootprintCandle, FootprintCandlePosition,
    FootprintData, FootprintDataType, FootprintGroupingMode, FootprintLevel, FootprintRenderMode,
    FootprintScaling, OutsideBarStyle, TextFormat,
};
pub use markers::{MarkerData, MarkerRenderConfig, MarkerShape, TradeMarker, TradeMarkerDebug};
pub use profile::{
    ExtendDirection, NodeDetectionMethod, ProfileLevel, ProfileOutput, ProfileRenderConfig,
    ProfileSide, VbpGroupingMode, VbpNodeConfig, VbpPeriod, VbpPocConfig, VbpResolvedCache,
    VbpSplitPeriod, VbpType, VbpValueAreaConfig, VbpVwapConfig, VolumeNode,
};
pub use series::{BarPoint, BarSeries, HistogramBar, LineSeries, PriceLevel};

/// Top-level enum of all study output variants.
#[derive(Debug, Clone, Default)]
pub enum StudyOutput {
    /// Single line series (SMA, EMA)
    Lines(Vec<LineSeries>),

    /// Multiple lines with optional fill between (Bollinger Bands)
    Band {
        upper: LineSeries,
        middle: Option<LineSeries>,
        lower: LineSeries,
        fill_opacity: f32,
    },

    /// Bar chart (Volume, Delta)
    Bars(Vec<BarSeries>),

    /// Histogram (MACD histogram)
    Histogram(Vec<HistogramBar>),

    /// Horizontal levels (Fibonacci, Support/Resistance)
    Levels(Vec<PriceLevel>),

    /// Price profile (Volume Profile, Market Profile, VBP)
    Profile(Vec<ProfileOutput>, ProfileRenderConfig),

    /// Footprint: per-candle trade-level data replacing standard
    /// candle rendering
    Footprint(FootprintData),

    /// Trade markers (Big Trades bubbles) with render configuration
    Markers(MarkerData),

    /// Multiple outputs combined (e.g. MACD: Lines + Histogram)
    Composite(Vec<StudyOutput>),

    /// No output yet (not computed)
    #[default]
    Empty,
}
