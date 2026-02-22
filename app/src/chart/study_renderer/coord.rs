//! Shared coordinate helpers for study renderers.
//!
//! Centralises value-range computation, value-to-pixel mapping,
//! effective line width, and line-dash conversion so that individual
//! renderers (line, band, bar, histogram, panel) do not duplicate them.

use iced::widget::canvas::LineDash;
use study::config::LineStyleValue;

/// Compute min/max for a set of f32 values with 5 % padding.
///
/// Returns `None` when the iterator is empty. When the range is zero
/// (all values equal), a fixed pad of 1.0 is used.
pub fn value_range(
    values: impl Iterator<Item = f32>,
) -> Option<(f32, f32)> {
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
        // Add small padding so lines don't sit on the edge
        let range = max - min;
        let pad = if range > 0.0 { range * 0.05 } else { 1.0 };
        Some((min - pad, max + pad))
    }
}

/// Map a value to a Y pixel coordinate within a panel.
///
/// `min`/`max` define the value range; `height` is the pixel height.
/// Returns 0 at max, `height` at min (screen Y increases downward).
pub fn value_to_panel_y(
    value: f32,
    min: f32,
    max: f32,
    height: f32,
) -> f32 {
    if max <= min {
        height
    } else {
        height - ((value - min) / (max - min)) * height
    }
}

/// Compute an effective line width that stays constant in screen pixels
/// regardless of the current zoom scaling.
#[inline]
pub fn effective_line_width(width: f32, scaling: f32) -> f32 {
    if scaling > f32::EPSILON {
        width / scaling
    } else {
        width
    }
}

/// Convert a `LineStyleValue` to an iced `LineDash`.
pub fn line_dash_for_style(
    style: &LineStyleValue,
) -> LineDash<'static> {
    match style {
        LineStyleValue::Solid => LineDash::default(),
        LineStyleValue::Dashed => LineDash {
            segments: &[6.0, 4.0],
            offset: 0,
        },
        LineStyleValue::Dotted => LineDash {
            segments: &[2.0, 3.0],
            offset: 0,
        },
    }
}
