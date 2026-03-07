//! Pure math helpers for study renderers.
//!
//! Value-range computation, value-to-pixel mapping, effective line
//! width, and dynamic quantum calculation — all independent of any
//! GUI framework.

/// Compute min/max for a set of f32 values with 5% padding.
///
/// Returns `None` when the iterator is empty. When the range is zero
/// (all values equal), a fixed pad of 1.0 is used.
pub fn value_range(values: impl Iterator<Item = f32>) -> Option<(f32, f32)> {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    let mut count = 0u32;

    for v in values {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
        count += 1;
    }

    if count == 0 {
        None
    } else {
        let range = max - min;
        let pad = if range > 0.0 { range * 0.05 } else { 1.0 };
        Some((min - pad, max + pad))
    }
}

/// Map a value to a Y pixel coordinate within a panel.
///
/// `min`/`max` define the value range; `height` is the pixel height.
/// Returns 0 at max, `height` at min (screen Y increases downward).
pub fn value_to_panel_y(value: f32, min: f32, max: f32, height: f32) -> f32 {
    if max <= min {
        height
    } else {
        height - ((value - min) / (max - min)) * height
    }
}

/// Compute an effective line width that stays constant in screen
/// pixels regardless of the current zoom scaling.
#[inline]
pub fn effective_line_width(width: f32, scaling: f32) -> f32 {
    if scaling > f32::EPSILON {
        width / scaling
    } else {
        width
    }
}

/// Compute the dynamic grouping quantum for automatic mode.
///
/// `min_row_px` is the minimum row height in screen pixels
/// (e.g. 4.0 for VBP bars, 16.0 for footprint text).
/// `factor` is the user's scale factor; larger values produce
/// coarser grouping. `tick_units` is the instrument tick size
/// in price units.
pub fn compute_dynamic_quantum(
    cell_height: f32,
    scaling: f32,
    min_row_px: f32,
    factor: i64,
    tick_units: i64,
) -> i64 {
    let pixel_per_tick = cell_height * scaling;
    let base_ticks = (min_row_px / pixel_per_tick).ceil() as i64;
    (base_ticks * factor).max(1) * tick_units
}
