//! Chart Traits
//!
//! Core traits that all chart implementations must implement.

use super::ViewState;
use crate::chart::Message;
use data::Indicator;
use iced::{Element, Vector, widget::canvas};

/// Core trait for all chart types
///
/// Provides common interface for chart operations including state access,
/// cache invalidation, and indicator rendering.
pub trait Chart: PlotConstants + canvas::Program<Message> {
    /// The indicator type this chart supports
    type IndicatorKind: Indicator;

    /// Get immutable reference to chart state
    fn state(&self) -> &ViewState;

    /// Get mutable reference to chart state
    fn mut_state(&mut self) -> &mut ViewState;

    /// Invalidate all rendering caches (main, labels, crosshair)
    fn invalidate_all(&mut self);

    /// Invalidate only the crosshair cache
    fn invalidate_crosshair(&mut self);

    /// Render indicator elements for this chart
    fn view_indicators(&'_ self, enabled: &[Self::IndicatorKind]) -> Vec<Element<'_, Message>>;

    /// Get the visible time range as (start, end) timestamps
    fn visible_timerange(&self) -> Option<(u64, u64)>;

    /// Get interval keys for tick-based charts
    fn interval_keys(&self) -> Option<Vec<u64>>;

    /// Calculate autoscaled coordinates based on current data
    fn autoscaled_coords(&self) -> Vector;

    /// Whether this chart supports fit-all autoscaling mode
    fn supports_fit_autoscaling(&self) -> bool;

    /// Check if the chart has no data to display
    fn is_empty(&self) -> bool;
}

/// Constants for chart scaling and sizing
///
/// Each chart type can define its own limits for zooming and cell sizes.
pub trait PlotConstants {
    /// Minimum scaling factor (most zoomed out)
    fn min_scaling(&self) -> f32;

    /// Maximum scaling factor (most zoomed in)
    fn max_scaling(&self) -> f32;

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
