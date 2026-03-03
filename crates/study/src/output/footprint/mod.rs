//! Footprint chart data structures and rendering configuration.
//!
//! - [`data`] — [`FootprintData`], [`FootprintCandle`], [`FootprintLevel`].
//! - [`render`] — [`CandleRenderConfig`] and mode/data-type/position enums.
//! - [`scaling`] — [`FootprintScaling`] strategies (sqrt, log, hybrid, etc.).

pub mod data;
pub mod render;
pub mod scaling;

pub use data::{FootprintCandle, FootprintData, FootprintLevel};
pub use render::{
    BackgroundColorMode, CandleRenderConfig, FootprintCandlePosition, FootprintDataType,
    FootprintGroupingMode, FootprintRenderMode, OutsideBarStyle, TextFormat,
};
pub use scaling::FootprintScaling;
