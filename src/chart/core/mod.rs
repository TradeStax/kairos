//! Core Chart Infrastructure
//!
//! Contains shared types and traits used by all chart implementations:
//! - `ViewState` - Chart view state (translation, scaling, bounds)
//! - `Caches` - Canvas rendering caches
//! - `Interaction` - User interaction state
//! - `Chart` trait - Common interface for all chart types
//! - `PlotConstants` trait - Chart-specific constants

mod caches;
mod interaction;
#[allow(dead_code)]
pub mod tokens;
mod traits;
mod view_state;

pub use caches::Caches;
pub use interaction::{ChartState, Interaction, canvas_interaction};
pub use traits::{Chart, PlotConstants};
pub use view_state::ViewState;
