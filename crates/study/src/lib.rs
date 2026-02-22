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

pub use config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterSection, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};
pub use error::StudyError;
pub use output::{
    CandleRenderConfig, FootprintCandle, FootprintCandlePosition, FootprintData,
    FootprintDataType, FootprintGroupingMode, FootprintLevel, FootprintRenderMode,
    FootprintScaling, MarkerData, MarkerRenderConfig, MarkerShape, StudyOutput, VbpData,
    VbpGroupingMode, VbpLengthUnit, VbpPeriod, VbpType,
};
pub use registry::{StudyInfo, StudyRegistry};
pub use traits::{Study, StudyCategory, StudyInput, StudyPlacement};
