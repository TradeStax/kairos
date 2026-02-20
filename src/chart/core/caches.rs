//! Canvas Rendering Caches
//!
//! Manages iced canvas caches for efficient rendering.

use iced::widget::canvas::Cache;

/// Collection of canvas caches for chart rendering
///
/// Separates different rendering layers for independent invalidation:
/// - `main` - Primary chart content (candles, depth, etc.)
/// - `drawings` - Completed drawing annotations
/// - `x_labels` - X-axis time/tick labels
/// - `y_labels` - Y-axis price labels
/// - `crosshair` - Crosshair overlay (selection handles, pending preview, crosshair)
#[derive(Default)]
pub struct Caches {
    /// Main chart content cache
    pub main: Cache,
    /// Completed drawings cache
    pub drawings: Cache,
    /// X-axis labels cache
    pub x_labels: Cache,
    /// Y-axis labels cache
    pub y_labels: Cache,
    /// Crosshair overlay cache
    pub crosshair: Cache,
}

impl Caches {
    /// Create new empty caches
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all caches (full redraw needed)
    pub fn clear_all(&self) {
        self.main.clear();
        self.drawings.clear();
        self.x_labels.clear();
        self.y_labels.clear();
        self.crosshair.clear();
    }

    /// Clear only the drawings cache
    pub fn clear_drawings(&self) {
        self.drawings.clear();
    }

    /// Clear crosshair and axis label caches
    ///
    /// Used when cursor moves but chart data hasn't changed.
    /// Axis labels must also be cleared because the `AxisLabelsX` and
    /// `AxisLabelsY` draw closures render the cursor-position label
    /// (crosshair price / time) directly into their respective caches.
    pub fn clear_crosshair(&self) {
        self.crosshair.clear();
        self.y_labels.clear();
        self.x_labels.clear();
    }

    /// Clear only main content cache
    pub fn clear_main(&self) {
        self.main.clear();
    }

    /// Clear only axis labels caches
    pub fn clear_labels(&self) {
        self.x_labels.clear();
        self.y_labels.clear();
    }
}
