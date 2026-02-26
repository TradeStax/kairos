//! State Persistence — Application State Serialization
//!
//! This module provides proper state management with clear separation:
//! - Application state (theme, UI preferences, window specs)
//! - Layout persistence (pane tree structures, splits)
//! - Persistence layer (load/save with versioning)
//!
//! ## Architecture
//!
//! ```text
//! AppState (persisted to app-state.json)
//!   |-- UI preferences (theme, timezone, scale)
//!   |-- Window configuration
//!   +-- Layout management
//!
//! Persistence Layer
//!   |-- Versioned serialization
//!   |-- Migration support
//!   +-- Backup on failure
//! ```

pub mod app_state;
pub mod layout;
pub mod loading;
#[allow(clippy::module_inception)]
pub mod persistence;
pub mod runtime;

pub use app_state::{AiPreferences, AppState, DatabentoAppConfig, WindowSpec};
pub use layout::{Axis, Dashboard, Layout, LayoutManager, Layouts, Pane};
pub use loading::load_saved_state_without_registry;
pub use persistence::{PersistenceError, StateVersion, load_state, save_state};
pub use runtime::{Layout as RuntimeLayout, LayoutId, SavedState, configuration};
