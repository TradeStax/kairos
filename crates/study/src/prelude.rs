//! Convenience prelude for implementing studies.
//!
//! A single `use kairos_study::prelude::*;` brings in the [`Study`] trait,
//! input/output types, the full config vocabulary (parameter definitions,
//! display formats, visibility rules, line styles), and the default
//! bull/bear color constants — everything needed to write a new study
//! implementation.
//!
//! ```ignore
//! use kairos_study::prelude::*;
//! ```

pub use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterSection, ParameterTab,
    ParameterValue, StudyConfig, Visibility,
};
pub use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
pub use crate::error::StudyError;
pub use crate::output::StudyOutput;
pub use crate::{BEARISH_COLOR, BULLISH_COLOR};
