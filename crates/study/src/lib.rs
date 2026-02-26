//! Technical studies and indicators for Kairos charts.
//!
//! This crate provides a trait-based computation system that transforms market
//! data (candles, trades) into abstract render primitives. The chart rendering
//! layer in the app crate converts these primitives into canvas draw calls.
//!
//! # Modules
//!
//! - [`config`] — Parameter definitions, validation, and runtime storage.
//! - [`core`] — The [`Study`] trait and its input/metadata types.
//! - [`output`] — Render primitives: lines, bars, profiles, footprints, markers.
//! - [`studies`] — Built-in study implementations and the [`StudyRegistry`] factory.
//! - [`util`] — Shared helpers for candle extraction and statistics.
//! - [`error`] — [`StudyError`] with severity classification.

pub mod config;
pub mod core;
pub mod error;
pub mod output;
pub mod prelude;
pub mod studies;
pub mod util;

pub use studies::orderflow;

/// Default bullish color (buy/ask) — #51CDA0.
pub const BULLISH_COLOR: data::SerializableColor =
    data::SerializableColor::from_rgb8_const(81, 205, 160);

/// Default bearish color (sell/bid) — #C0504D.
pub const BEARISH_COLOR: data::SerializableColor =
    data::SerializableColor::from_rgb8_const(192, 80, 77);

// --- Config re-exports ---
pub use config::LineStyleValue;
pub use config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterSection, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};

// --- Core re-exports ---
pub use core::{Study, StudyCategory, StudyInput, StudyPlacement};
pub use error::StudyError;

// --- Output re-exports ---
pub use output::{
    BackgroundColorMode, BarPoint, BarSeries, CandleRenderConfig, ExtendDirection, FootprintCandle,
    FootprintCandlePosition, FootprintData, FootprintDataType, FootprintGroupingMode,
    FootprintLevel, FootprintRenderMode, FootprintScaling, HistogramBar, LineSeries, MarkerData,
    MarkerRenderConfig, MarkerShape, NodeDetectionMethod, OutsideBarStyle, PriceLevel,
    ProfileLevel, ProfileOutput, ProfileRenderConfig, ProfileSide, StudyOutput, TextFormat,
    TradeMarker, TradeMarkerDebug, VbpGroupingMode, VbpNodeConfig, VbpPeriod, VbpPocConfig,
    VbpSplitPeriod, VbpType, VbpValueAreaConfig, VbpVwapConfig, VolumeNode,
};

// --- Registry re-exports ---
pub use studies::{StudyInfo, StudyRegistry};
