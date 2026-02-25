//! Kairos Study Crate
//!
//! Technical studies and indicators for Kairos charts.
//! Provides a trait-based system for computing and outputting
//! study data as abstract render primitives.

pub mod config;
pub mod core;
pub mod error;
pub mod output;
pub mod prelude;
pub mod registry;
pub mod util;

// Study implementations
pub mod momentum;
pub mod orderflow;
pub mod trend;
pub mod volatility;
pub mod volume;

/// Default bullish/buy/ask color — matches theme success (#51CDA0).
pub const BULLISH_COLOR: data::SerializableColor =
    data::SerializableColor::from_rgb8_const(81, 205, 160);
/// Default bearish/sell/bid color — matches theme danger (#C0504D).
pub const BEARISH_COLOR: data::SerializableColor =
    data::SerializableColor::from_rgb8_const(192, 80, 77);

pub use config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterSection, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};
pub use config::LineStyleValue;
pub use core::{Study, StudyCategory, StudyInput, StudyPlacement};
pub use error::StudyError;
pub use output::{
    BackgroundColorMode, CandleRenderConfig, FootprintCandle, FootprintCandlePosition,
    FootprintData, FootprintDataType, FootprintGroupingMode,
    FootprintLevel, FootprintRenderMode, FootprintScaling,
    MarkerData, MarkerRenderConfig, MarkerShape, TradeMarker, TradeMarkerDebug,
    OutsideBarStyle, TextFormat,
    ProfileOutput, ProfileRenderConfig, StudyOutput,
    VbpGroupingMode, VbpPeriod, VbpSplitPeriod, VbpType,
    VbpPocConfig, VbpValueAreaConfig, VbpNodeConfig, VbpVwapConfig,
    ExtendDirection, NodeDetectionMethod, ProfileLevel, ProfileSide, VolumeNode,
    BarPoint, BarSeries, HistogramBar, LineSeries, PriceLevel,
};
pub use registry::{StudyInfo, StudyRegistry};
