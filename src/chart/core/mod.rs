//! Core Chart Infrastructure
//!
//! Contains shared types and traits used by all chart implementations:
//! - `ViewState` - Chart view state (translation, scaling, bounds)
//! - `Caches` - Canvas rendering caches
//! - `Interaction` - User interaction state
//! - `Chart` trait - Common interface for all chart types
//! - `PlotConstants` trait - Chart-specific constants

mod autoscale;
mod caches;
mod interaction;
mod traits;
mod view_state;

pub use autoscale::*;
pub use caches::Caches;
pub use interaction::{DrawingEditMode, DrawingState, Interaction, canvas_interaction};
pub use traits::{Chart, PlotConstants};
pub use view_state::ViewState;
