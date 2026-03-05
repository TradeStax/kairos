//! Service Layer — Application-level services
//!
//! Services that coordinate between the data engine and the UI layer.

pub mod error;
pub mod replay;
pub mod trade_provider;
pub mod updater;
pub mod updater_install;

pub use replay::{ReplayEngine, ReplayEngineConfig, ReplayEvent, VolumeBucket};
