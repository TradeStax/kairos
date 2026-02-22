//! Chart Traits
//!
//! Core traits that all chart implementations must implement.

use super::ViewState;
use crate::chart::Message;
use data::DrawingId;
use iced::widget::canvas::{self, Cache};
use iced::{Point, Size, Vector};

/// Info about a single panel study for rendering below the main chart.
pub struct PanelStudyInfo<'a> {
    pub name: &'a str,
    pub output: &'a study::StudyOutput,
}

/// Core trait for all chart types
///
/// Provides common interface for chart operations including state access,
/// cache invalidation, and rendering.
pub trait Chart: PlotConstants + canvas::Program<Message> {
    /// Get immutable reference to chart state
    fn state(&self) -> &ViewState;

    /// Get mutable reference to chart state
    fn mut_state(&mut self) -> &mut ViewState;

    /// Invalidate all rendering caches (main, labels, crosshair)
    fn invalidate_all(&mut self);

    /// Invalidate only the crosshair cache
    fn invalidate_crosshair(&mut self);

    /// Get interval keys for tick-based charts
    fn interval_keys(&self) -> Option<Vec<u64>>;

    /// Calculate autoscaled coordinates based on current data
    fn autoscaled_coords(&self) -> Vector;

    /// Whether this chart supports fit-all autoscaling mode
    fn supports_fit_autoscaling(&self) -> bool;

    /// Check if the chart has no data to display
    fn is_empty(&self) -> bool;

    /// Get the active drawing tool from the chart's DrawingManager
    ///
    /// Returns `DrawingTool::None` by default. Chart implementations with
    /// a `DrawingManager` should override this to return the active tool.
    fn active_drawing_tool(&self) -> data::DrawingTool {
        data::DrawingTool::None
    }

    /// Check if there is a pending (in-progress) drawing
    fn has_pending_drawing(&self) -> bool {
        false
    }

    /// Hit test all drawings at a screen point, returning the topmost hit
    fn hit_test_drawing(&self, _screen_point: Point, _bounds: Size) -> Option<DrawingId> {
        None
    }

    /// Hit test selection handles on already-selected drawings
    fn hit_test_drawing_handle(
        &self,
        _screen_point: Point,
        _bounds: Size,
    ) -> Option<(DrawingId, usize)> {
        None
    }

    /// Check if any drawing is currently selected
    fn has_drawing_selection(&self) -> bool {
        false
    }

    /// Check if a specific drawing is currently selected
    fn is_drawing_selected(&self, _id: DrawingId) -> bool {
        false
    }

    /// Check if a specific drawing is locked
    fn is_drawing_locked(&self, _id: DrawingId) -> bool {
        false
    }

    /// Check if a clone placement is in progress
    fn has_clone_pending(&self) -> bool {
        false
    }

    /// Get info for panel-placement studies to render below the chart.
    fn panel_studies(&self) -> Vec<PanelStudyInfo<'_>> {
        Vec::new()
    }

    /// Get the cache used for panel study rendering.
    fn panel_cache(&self) -> Option<&Cache> {
        None
    }

    /// Get the cache used for panel Y-axis label rendering.
    fn panel_labels_cache(&self) -> Option<&Cache> {
        None
    }
}

/// Constants for chart scaling and sizing
///
/// Each chart type can define its own limits for zooming and cell sizes.
pub trait PlotConstants {
    /// Maximum cell width in pixels
    fn max_cell_width(&self) -> f32;

    /// Minimum cell width in pixels
    fn min_cell_width(&self) -> f32;

    /// Maximum cell height in pixels
    fn max_cell_height(&self) -> f32;

    /// Minimum cell height in pixels
    fn min_cell_height(&self) -> f32;

    /// Default cell width for reset operations
    fn default_cell_width(&self) -> f32;
}
