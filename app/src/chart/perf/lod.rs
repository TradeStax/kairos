//! Level-of-Detail (LOD) System
//!
//! Dynamically adjusts rendering quality based on zoom level, data density,
//! and viewport size to maintain 60 FPS performance.
//!
//! ## Architecture
//!
//! ```text
//! Viewport State (zoom, data density)
//!     ↓
//! LOD Calculator
//!     ↓
//! LOD Level (0=lowest, 2=highest detail)
//!     ↓
//! Decimation Factor (1, 3, 10, etc.)
//!     ↓
//! Rendering (skip every Nth item)
//! ```

/// Level-of-Detail level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LodLevel {
    /// Lowest detail - maximum decimation (zoomed far out)
    Low = 0,
    /// Medium detail - moderate decimation (normal view)
    Medium = 1,
    /// High detail - no decimation (zoomed in)
    High = 2,
}

impl LodLevel {
    /// Get decimation factor (render every Nth item)
    pub fn decimation_factor(&self) -> usize {
        match self {
            LodLevel::Low => 10,   // Render every 10th item
            LodLevel::Medium => 3, // Render every 3rd item
            LodLevel::High => 1,   // Render all items
        }
    }

    /// Should render text labels at this LOD level
    #[allow(dead_code)] // planned: used by future culling pass
    pub fn show_text(&self) -> bool {
        matches!(self, LodLevel::High)
    }

    /// Should render fine details (small circles, thin lines)
    #[allow(dead_code)] // planned: used by future culling pass
    pub fn show_fine_details(&self) -> bool {
        !matches!(self, LodLevel::Low)
    }

    /// Get maximum safe render count for this LOD level
    #[allow(dead_code)] // planned: used by future culling pass
    pub fn max_render_count(&self) -> usize {
        match self {
            LodLevel::Low => 1_000,
            LodLevel::Medium => 5_000,
            LodLevel::High => 20_000,
        }
    }
}

/// Scaling threshold below which we force Low LOD (very zoomed out).
const LOW_LOD_SCALING_THRESHOLD: f32 = 0.5;
/// Scaling threshold below which we force Medium LOD (moderately zoomed out).
const MEDIUM_LOD_SCALING_THRESHOLD: f32 = 1.0;
/// Items-per-pixel density above which we force Low LOD.
const LOW_LOD_DENSITY_THRESHOLD: f32 = 5.0;
/// Items-per-pixel density above which we force Medium LOD.
const MEDIUM_LOD_DENSITY_THRESHOLD: f32 = 2.0;
/// Visible item count above which we force Low LOD.
const LOW_LOD_ITEM_COUNT_THRESHOLD: usize = 10_000;
/// Visible item count above which we force Medium LOD.
const MEDIUM_LOD_ITEM_COUNT_THRESHOLD: usize = 5_000;

/// LOD calculator based on viewport and data characteristics
pub struct LodCalculator {
    /// Current scaling factor (zoom level)
    scaling: f32,
    /// Cell width in viewport pixels
    _cell_width_pixels: f32,
    /// Number of visible items (trades, candles, etc.)
    visible_item_count: usize,
    /// Viewport width in pixels
    viewport_width: f32,
}

impl LodCalculator {
    /// Create new LOD calculator
    pub fn new(
        scaling: f32,
        cell_width: f32,
        visible_item_count: usize,
        viewport_width: f32,
    ) -> Self {
        let cell_width_pixels = cell_width * scaling;
        Self {
            scaling,
            _cell_width_pixels: cell_width_pixels,
            visible_item_count,
            viewport_width,
        }
    }

    /// Get effective decimation for a given max item budget.
    ///
    /// Returns how many items to skip so the total rendered count
    /// stays within `max_items`.
    #[allow(dead_code)] // used by heatmap feature
    pub fn effective_decimation(&self, max_items: usize) -> usize {
        if max_items == 0 || self.visible_item_count <= max_items {
            return 1;
        }
        self.visible_item_count.div_ceil(max_items)
    }

    /// Calculate appropriate LOD level
    ///
    /// Decision factors:
    /// 1. Zoom level (scaling factor)
    /// 2. Data density (items per pixel)
    /// 3. Item count vs render budget
    pub fn calculate_lod(&self) -> LodLevel {
        // Factor 1: Items per pixel (data density)
        let items_per_pixel = if self.viewport_width > 0.0 {
            self.visible_item_count as f32 / self.viewport_width
        } else {
            0.0
        };

        // Factor 2: Cell width in pixels (zoom indicator)
        // Small cells = zoomed out, Large cells = zoomed in

        // Factor 3: Total item count
        let item_count = self.visible_item_count;

        // Decision logic
        if self.scaling < LOW_LOD_SCALING_THRESHOLD
            || items_per_pixel > LOW_LOD_DENSITY_THRESHOLD
            || item_count > LOW_LOD_ITEM_COUNT_THRESHOLD
        {
            LodLevel::Low
        } else if self.scaling < MEDIUM_LOD_SCALING_THRESHOLD
            || items_per_pixel > MEDIUM_LOD_DENSITY_THRESHOLD
            || item_count > MEDIUM_LOD_ITEM_COUNT_THRESHOLD
        {
            LodLevel::Medium
        } else {
            LodLevel::High
        }
    }
}

/// LOD-aware iterator that skips items based on decimation factor
pub struct LodIterator<I> {
    inner: I,
    decimation: usize,
    index: usize,
}

impl<I> LodIterator<I> {
    pub fn new(inner: I, decimation: usize) -> Self {
        Self {
            inner,
            decimation: decimation.max(1),
            index: 0,
        }
    }
}

impl<I: Iterator> Iterator for LodIterator<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.inner.next()?;
            let should_render = self.index.is_multiple_of(self.decimation);
            self.index += 1;

            if should_render {
                return Some(item);
            }
        }
    }
}

/// Extension trait to add LOD filtering to iterators
pub trait LodIteratorExt: Iterator + Sized {
    /// Apply LOD decimation to this iterator
    fn lod_filter(self, decimation: usize) -> LodIterator<Self> {
        LodIterator::new(self, decimation)
    }
}

impl<I: Iterator> LodIteratorExt for I {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lod_decimation_factors() {
        assert_eq!(LodLevel::Low.decimation_factor(), 10);
        assert_eq!(LodLevel::Medium.decimation_factor(), 3);
        assert_eq!(LodLevel::High.decimation_factor(), 1);
    }

    #[test]
    fn test_lod_calculation_zoomed_out() {
        let calc = LodCalculator::new(0.3, 4.0, 15_000, 800.0);
        assert_eq!(calc.calculate_lod(), LodLevel::Low);
    }

    #[test]
    fn test_lod_calculation_normal() {
        let calc = LodCalculator::new(0.8, 4.0, 3_000, 800.0);
        assert_eq!(calc.calculate_lod(), LodLevel::Medium);
    }

    #[test]
    fn test_lod_calculation_zoomed_in() {
        let calc = LodCalculator::new(1.5, 10.0, 500, 800.0);
        assert_eq!(calc.calculate_lod(), LodLevel::High);
    }

    #[test]
    fn test_lod_iterator() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let result: Vec<_> = data.into_iter().lod_filter(3).collect();
        assert_eq!(result, vec![1, 4, 7, 10]); // Every 3rd item
    }
}
