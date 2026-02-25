//! Prelude for implementing custom studies.
//!
//! Import all essential types with a single glob:
//! ```rust
//! use kairos_study::prelude::*;
//! ```

pub use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
pub use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterSection,
    ParameterTab, ParameterValue, StudyConfig, Visibility,
};
pub use crate::output::StudyOutput;
pub use crate::error::StudyError;
pub use crate::{BULLISH_COLOR, BEARISH_COLOR};
