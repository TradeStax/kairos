//! Kairos Script Crate
//!
//! JavaScript scripting engine for Kairos indicators.
//! Allows users to define custom indicators in `.js` files
//! using a PineScript-inspired API, powered by QuickJS.

pub mod bridge;
pub mod compiler;
pub mod engine;
mod path;
pub mod error;
pub mod loader;
pub mod manifest;
pub mod registry;
pub mod runtime;
pub mod study_adapter;

pub use engine::ScriptEngine;
pub use error::ScriptError;
pub use loader::ScriptLoader;
pub use manifest::ScriptManifest;
pub use registry::ScriptRegistry;
pub use study_adapter::ScriptStudy;
