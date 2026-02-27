//! Chart Traits
//!
//! Core traits that all chart implementations must implement.

use super::ViewState;
use super::tokens;
use crate::chart::Message;
use crate::chart::drawing::DrawingManager;
use crate::drawing::DrawingId;
use iced::widget::canvas::{self, Cache};
use iced::{Point, Size, Vector};

/// Info about a single panel study for rendering below the main chart.
pub struct PanelStudyInfo<'a> {
    pub name: &'a str,
    pub output: &'a study::StudyOutput,
}

/// Info about a single side-panel study for rendering to the right of the chart.
pub struct SidePanelStudyInfo<'a> {
    pub output: &'a study::StudyOutput,
}

/// Constants for chart scaling and sizing.
///
/// Each chart type returns its own limits for zooming and cell sizes.
#[derive(Debug, Clone, Copy)]
pub struct PlotLimits {
    pub max_cell_width: f32,
    pub min_cell_width: f32,
    pub max_cell_height: f32,
    pub min_cell_height: f32,
    pub default_cell_width: f32,
}

/// Core trait for all chart types
///
/// Provides common interface for chart operations including state access,
/// cache invalidation, and rendering.
pub trait Chart: canvas::Program<Message> {
    /// Get immutable reference to chart state
    fn state(&self) -> &ViewState;

    /// Get mutable reference to chart state
    fn mut_state(&mut self) -> &mut ViewState;

    /// Invalidate all rendering caches (main, labels, crosshair)
    fn invalidate_all(&mut self);

    /// Invalidate rendering caches for view-only changes (zoom, pan).
    ///
    /// Lighter than [`invalidate_all`] — clears caches and handles
    /// autoscale but skips study recomputation. Use for high-frequency
    /// viewport changes where the underlying data hasn't changed.
    fn invalidate_view(&mut self) {
        self.invalidate_all();
    }

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

    /// Get scaling and sizing limits for this chart type.
    fn plot_limits(&self) -> PlotLimits;

    // ── Drawing accessor ──────────────────────────────────────────

    /// Get immutable reference to drawing manager (if chart supports
    /// drawings). Override to return `Some(&self.drawings)`.
    fn drawings(&self) -> Option<&DrawingManager> {
        None
    }

    // ── Drawing methods (default impls via drawings() accessor) ───

    /// Get the active drawing tool from the chart's DrawingManager.
    fn active_drawing_tool(&self) -> crate::drawing::DrawingTool {
        self.drawings()
            .map(|d| d.active_tool())
            .unwrap_or(crate::drawing::DrawingTool::None)
    }

    /// Check if there is a pending (in-progress) drawing.
    fn has_pending_drawing(&self) -> bool {
        self.drawings().is_some_and(|d| d.has_pending())
    }

    /// Hit test all drawings at a screen point, returning the
    /// topmost hit.
    fn hit_test_drawing(&self, screen_point: Point, bounds: Size) -> Option<DrawingId> {
        self.drawings().and_then(|d| {
            d.hit_test(
                screen_point,
                self.state(),
                bounds,
                tokens::drawing::HIT_TOLERANCE,
            )
        })
    }

    /// Hit test selection handles on already-selected drawings.
    fn hit_test_drawing_handle(
        &self,
        screen_point: Point,
        bounds: Size,
    ) -> Option<(DrawingId, usize)> {
        self.drawings().and_then(|d| {
            d.hit_test_handle(
                screen_point,
                self.state(),
                bounds,
                tokens::drawing::HANDLE_SIZE,
            )
        })
    }

    /// Check if any drawing is currently selected.
    fn has_drawing_selection(&self) -> bool {
        self.drawings()
            .is_some_and(|d| !d.selected_ids().is_empty())
    }

    /// Check if a specific drawing is currently selected.
    fn is_drawing_selected(&self, id: DrawingId) -> bool {
        self.drawings().is_some_and(|d| d.is_selected(id))
    }

    /// Check if a specific drawing is locked.
    fn is_drawing_locked(&self, id: DrawingId) -> bool {
        self.drawings()
            .and_then(|d| d.get(id))
            .is_some_and(|d| d.locked)
    }

    /// Check if a clone placement is in progress.
    fn has_clone_pending(&self) -> bool {
        self.drawings().is_some_and(|d| d.has_clone_pending())
    }

    // ── Study overlay hit testing ────────────────────────────────

    /// Hit-test the study overlay text labels at the given screen
    /// point. Returns the study index if a label was hit.
    fn hit_test_study_overlay(&self, _point: Point) -> Option<usize> {
        None
    }

    /// Hit-test the detail icon buttons next to study overlay labels.
    /// Returns the study index if a detail button was hit.
    fn hit_test_study_detail_button(&self, _point: Point) -> Option<usize> {
        None
    }

    // ── Panel study methods ───────────────────────────────────────

    /// Get all studies attached to this chart.
    fn studies(&self) -> &[Box<dyn study::Study>] {
        &[]
    }

    /// Get info for panel-placement studies to render below the
    /// chart.
    fn panel_studies(&self) -> Vec<PanelStudyInfo<'_>> {
        self.studies()
            .iter()
            .filter(|s| s.placement() == study::StudyPlacement::Panel)
            .filter(|s| !matches!(s.output(), study::StudyOutput::Empty))
            .map(|s| PanelStudyInfo {
                name: s.name(),
                output: s.output(),
            })
            .collect()
    }

    /// Get the cache used for panel study rendering.
    fn panel_cache(&self) -> Option<&Cache> {
        None
    }

    /// Get the cache used for panel Y-axis label rendering.
    fn panel_labels_cache(&self) -> Option<&Cache> {
        None
    }

    /// Get the cache used for the panel crosshair overlay.
    fn panel_crosshair_cache(&self) -> Option<&Cache> {
        None
    }

    // ── Side panel study methods ──────────────────────────────────

    /// Get info for side-panel-placement studies rendered to the
    /// right of the main chart, sharing the price Y-axis.
    fn side_panel_studies(&self) -> Vec<SidePanelStudyInfo<'_>> {
        self.studies()
            .iter()
            .filter(|s| s.placement() == study::StudyPlacement::SidePanel)
            .filter(|s| !matches!(s.output(), study::StudyOutput::Empty))
            .map(|s| SidePanelStudyInfo { output: s.output() })
            .collect()
    }

    /// Get the cache used for side panel content rendering.
    fn side_panel_cache(&self) -> Option<&Cache> {
        None
    }

    /// Get the cache used for the side panel crosshair overlay.
    fn side_panel_crosshair_cache(&self) -> Option<&Cache> {
        None
    }
}
