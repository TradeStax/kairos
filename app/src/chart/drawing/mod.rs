//! Drawing Module
//!
//! Provides drawing tools for chart annotations including lines, rays,
//! horizontal/vertical lines, rectangles, and trend lines.

#[allow(clippy::module_inception)]
mod drawing;
mod manager;
#[allow(dead_code)]
pub mod persistence;
mod point;
pub mod render;
pub mod snap;

pub use drawing::Drawing;
pub use manager::DrawingManager;
pub use point::DrawingPoint;

// Re-export types from data layer for convenience
pub use data::{DrawingTool, LineStyle};
