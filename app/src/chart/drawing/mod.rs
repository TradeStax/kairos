//! Drawing Module
//!
//! Provides drawing tools for chart annotations including lines, rays,
//! horizontal/vertical lines, rectangles, and trend lines.

#[allow(clippy::module_inception)]
mod drawing;
pub mod hit_test;
mod manager;
mod point;
pub mod render;
pub mod snap;

pub use drawing::Drawing;
pub use manager::DrawingManager;
pub use point::DrawingPoint;

// Re-export types from data layer for convenience
pub use data::{DrawingTool, LineStyle};

use super::ViewState;

/// Trait for accessing a chart's drawing system and related view state.
///
/// Both `KlineChart` and `HeatmapChart` implement this, enabling
/// `Content` to operate on drawings without duplicating match arms
/// for each chart type.
pub trait ChartDrawingAccess {
    /// Immutable access to the drawing manager.
    fn drawings(&self) -> &DrawingManager;

    /// Mutable access to the drawing manager.
    ///
    /// Unlike the inherent `drawings_mut()` on chart types, this does
    /// **not** auto-clear any caches — call the invalidation methods
    /// explicitly.
    fn drawings_mut(&mut self) -> &mut DrawingManager;

    /// Immutable access to the chart view state.
    fn view_state(&self) -> &ViewState;

    /// Invalidate the drawings rendering cache.
    fn invalidate_drawings_cache(&mut self);

    /// Invalidate the crosshair/overlay rendering cache.
    fn invalidate_crosshair_cache(&mut self);
}
