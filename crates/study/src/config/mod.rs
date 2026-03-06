//! Study parameter definitions, validation, and runtime storage.
//!
//! Each study declares its configurable parameters as [`ParameterDef`] values.
//! At runtime, the current parameter values live in a [`StudyConfig`] and are
//! validated against the definitions before being applied.
//!
//! - `display` — Formatting and conditional visibility rules for the UI.
//! - `parameter` — Parameter definitions, kinds, tabs, and sections.
//! - `store` — Runtime config storage with typed getters.
//! - `value` — Concrete parameter value types (int, float, color, etc.).

mod display;
mod parameter;
mod store;
mod value;
pub mod versioning;

pub use display::{DisplayFormat, Visibility};
pub use parameter::{ParameterDef, ParameterKind, ParameterSection, ParameterTab};
pub use store::StudyConfig;
pub use value::{LineStyleValue, ParameterValue};

#[cfg(test)]
mod tests;
