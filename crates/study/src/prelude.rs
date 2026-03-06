//! Convenience prelude for implementing studies.
//!
//! A single `use kairos_study::prelude::*;` brings in the [`Study`] trait,
//! input/output types, the full config vocabulary (parameter definitions,
//! display formats, visibility rules, line styles), metadata types, result
//! types, capability traits, interactive types, and the default bull/bear
//! color constants — everything needed to write a new study implementation.
//!
//! ```ignore
//! use kairos_study::prelude::*;
//! ```

// Config
pub use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterSection, ParameterTab,
    ParameterValue, StudyConfig, Visibility,
};

// Core trait, metadata, input, result
pub use crate::core::{
    DiagnosticSeverity, Study, StudyCapabilities, StudyCategory, StudyDiagnostic, StudyInput,
    StudyMetadata, StudyPlacement, StudyResult, YScaleMode,
};

// Capability traits
pub use crate::core::capabilities::{
    CandleReplaceStudy, CompositeStudy, CustomDrawStudy, CustomTabStudy, ExternalDataStudy,
    IncrementalStudy, InteractiveStudy,
};

// Interactive types
pub use crate::core::interactive::{
    CrosshairValue, InteractivePayload, InteractiveRegion, StudyModalSpec,
};

// Composition
pub use crate::core::composition::{DependencyOutputs, StudyDependency};

// Error
pub use crate::error::StudyError;

// Output
pub use crate::output::StudyOutput;

// Constants
pub use crate::{BEARISH_COLOR, BULLISH_COLOR, NEUTRAL_COLOR};
