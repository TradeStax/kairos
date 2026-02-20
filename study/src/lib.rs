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

pub use config::{ParameterDef, ParameterKind, ParameterValue, StudyConfig};
pub use error::StudyError;
pub use output::StudyOutput;
pub use registry::{StudyInfo, StudyRegistry};
pub use traits::{Study, StudyCategory, StudyInput, StudyPlacement};
