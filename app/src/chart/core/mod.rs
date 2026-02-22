//! Core Chart Infrastructure
//!
//! Contains shared types and traits used by all chart implementations:
//! - `ViewState` - Chart view state (translation, scaling, bounds)
//! - `Caches` - Canvas rendering caches
//! - `Interaction` - User interaction state
//! - `Chart` trait - Common interface for all chart types
//! - `PlotLimits` - Chart-specific scaling constants

mod caches;
mod interaction;
pub mod tokens;
mod traits;
mod view_state;

pub use caches::Caches;
pub use interaction::{ChartState, Interaction, canvas_interaction};
pub use traits::{Chart, PanelStudyInfo, PlotLimits};
pub use view_state::{ViewState, x_to_interval};
