//! Core Chart Infrastructure
//!
//! Contains shared types and traits used by all chart implementations:
//! - `ViewState` - Chart view state (translation, scaling, bounds)
//! - `Caches` - Canvas rendering caches
//! - `Interaction` - User interaction state
//! - `Chart` trait - Common interface for all chart types
//! - `PlotLimits` - Chart-specific scaling constants

mod caches;
mod definition;
mod interaction;
pub(crate) mod macros;
pub mod tokens;
mod view_state;

pub use caches::Caches;
pub use definition::{Chart, PanelStudyInfo, PlotLimits, SidePanelStudyInfo};
pub use interaction::{ChartState, Interaction, base_mouse_interaction, canvas_interaction};
pub use view_state::{ViewState, x_to_interval};
