//! Convenience prelude for implementing studies.
//!
//! ```ignore
//! use kairos_study::prelude::*;
//! ```

pub use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterSection, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};
pub use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
pub use crate::error::StudyError;
pub use crate::output::StudyOutput;
pub use crate::{BEARISH_COLOR, BULLISH_COLOR};
