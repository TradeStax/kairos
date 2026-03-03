//! Abstract render primitives produced by studies.
//!
//! The chart rendering layer converts these into canvas draw calls. Each study
//! returns a [`StudyOutput`] variant matching its visualization type.
//!
//! - `series` — Line, bar, histogram, and price level primitives.
//! - `markers` — Trade marker bubbles (Big Trades study).
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
pub use series::{
    BarPoint, BarSeries, HistogramBar, LineSeries, PriceLevel, StudyCandlePoint, StudyCandleSeries,
};

/// Top-level enum of all study output variants.
///
/// Each study's `output()` method returns one of these variants. The chart
/// renderer pattern-matches on the variant to dispatch to the appropriate
/// drawing routine.
#[derive(Debug, Clone, Default)]
pub enum StudyOutput {
    /// One or more line series (e.g. SMA, EMA, VWAP).
    Lines(Vec<LineSeries>),

    /// Upper/lower band with optional midline and fill between them
    /// (e.g. Bollinger Bands).
    Band {
        /// Upper band line.
        upper: LineSeries,
        /// Optional middle line (e.g. the SMA in Bollinger Bands).
        middle: Option<LineSeries>,
        /// Lower band line.
        lower: LineSeries,
        /// Opacity of the shaded region between upper and lower bands.
        fill_opacity: f32,
    },

    /// Vertical bar chart (e.g. Volume, Delta).
    Bars(Vec<BarSeries>),

    /// Histogram bars below/above zero (e.g. MACD histogram).
    Histogram(Vec<HistogramBar>),

    /// Horizontal price levels (e.g. Fibonacci, Support/Resistance).
    Levels(Vec<PriceLevel>),

    /// Volume profile with rendering configuration.
    Profile(Vec<ProfileOutput>, ProfileRenderConfig),

    /// Footprint: per-candle trade-level data that replaces standard
    /// candle rendering.
    Footprint(FootprintData),

    /// Trade marker bubbles (Big Trades) with render configuration.
    Markers(MarkerData),

    /// OHLC mini-candlesticks (e.g. Speed of Tape).
    StudyCandles(Vec<StudyCandleSeries>),

    /// Multiple outputs combined (e.g. MACD: Lines + Histogram).
    Composite(Vec<StudyOutput>),

    /// No output yet (study has not been computed).
    #[default]
    Empty,
}
