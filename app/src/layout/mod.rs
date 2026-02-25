//! Layout management — types, loading, and save operations.
//!
//! ## Load path
//! [`load_saved_state_without_registry`] — reads `app-state.json` at startup.
//!
//! ## Save path
//! [`crate::app::layout::dashboard::Kairos::save_state_to_disk`] — writes on exit.
//! Located in `app/src/app/layout/dashboard.rs` alongside other runtime layout operations.

mod persistence;
mod types;

pub use persistence::*;
pub use types::*;
