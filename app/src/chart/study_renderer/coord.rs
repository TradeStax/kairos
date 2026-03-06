//! Shared coordinate helpers for study renderers.
//!
//! Centralises value-range computation, value-to-pixel mapping,
//! effective line width, and line-dash conversion so that individual
//! renderers (line, band, bar, histogram, panel) do not duplicate them.

use crate::chart::ViewState;
use iced::Color;
use iced::widget::canvas::LineDash;
use study::config::LineStyleValue;

/// Compute min/max for a set of f32 values with 5 % padding.
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
pub fn value_to_panel_y(value: f32, min: f32, max: f32, height: f32) -> f32 {
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
pub fn line_dash_for_style(style: &LineStyleValue) -> LineDash<'static> {
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

/// Compute the dynamic grouping quantum for automatic mode.
///
/// `min_row_px` is the minimum row height in screen pixels
/// (e.g. 4.0 for VBP bars, 16.0 for footprint text).
/// `factor` is the user's scale factor; larger values produce
/// coarser grouping. `tick_units` is the instrument tick size
/// in price units.
pub(crate) fn compute_dynamic_quantum(
    state: &ViewState,
    min_row_px: f32,
    factor: i64,
    tick_units: i64,
) -> i64 {
    let pixel_per_tick = state.cell_height * state.scaling;
    let base_ticks = (min_row_px / pixel_per_tick).ceil() as i64;
    (base_ticks * factor).max(1) * tick_units
}

#[allow(dead_code)] // Ready for use when studies declare non-Linear Y scales
/// Apply a Y-scale transformation to a value.
///
/// For `Linear` and `Percentage` modes this is an identity; for `Log10`
/// the value is log-transformed; for `Fixed` the value passes through
/// unchanged (the caller clamps via [`scaled_value_range`]).
pub fn apply_y_scale(value: f32, mode: study::YScaleMode) -> f32 {
    match mode {
        study::YScaleMode::Linear | study::YScaleMode::Percentage => value,
        study::YScaleMode::Log10 => value.max(f32::EPSILON).log10(),
        study::YScaleMode::Fixed { .. } => value,
    }
}

#[allow(dead_code)] // Ready for use when studies declare non-Linear Y scales
/// Compute the effective min/max after applying a Y-scale mode.
///
/// `Fixed` overrides the range entirely; other modes transform the
/// given min/max through [`apply_y_scale`].
pub fn scaled_value_range(min: f32, max: f32, mode: study::YScaleMode) -> (f32, f32) {
    match mode {
        study::YScaleMode::Fixed {
            min: fmin,
            max: fmax,
        } => (fmin, fmax),
        _ => (apply_y_scale(min, mode), apply_y_scale(max, mode)),
    }
}

/// Convert a `SerializableColor` to an iced `Color`, applying
/// an opacity multiplier to the alpha channel.
pub(crate) fn to_iced_color(sc: data::SerializableColor, opacity: f32) -> Color {
    Color {
        r: sc.r,
        g: sc.g,
        b: sc.b,
        a: sc.a * opacity,
    }
}
