//! Kairos Study Crate
//!
//! Technical studies and indicators for Kairos charts.
//! Provides a trait-based system for computing and outputting
//! study data as abstract render primitives.

pub mod config;
pub mod error;
pub mod output;
pub mod registry;
pub mod traits;
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
pub use error::StudyError;
pub use output::{
    CandleRenderConfig, FootprintCandle, FootprintCandlePosition,
    FootprintData, FootprintDataType, FootprintGroupingMode,
    FootprintLevel, FootprintRenderMode, FootprintScaling,
    MarkerData, MarkerRenderConfig, MarkerShape,
    ProfileOutput, ProfileRenderConfig, StudyOutput,
    VbpGroupingMode, VbpPeriod, VbpSplitPeriod, VbpType,
};
pub use registry::{StudyInfo, StudyRegistry};
pub use traits::{Study, StudyCategory, StudyInput, StudyPlacement};
