//! Band renderer.
//!
//! Renders Bollinger-style bands: upper line, optional middle line,
//! lower line, with a semi-transparent fill between upper and lower.

use super::super::canvas::Canvas;
use super::super::chart_view::ChartView;
use super::super::coord::effective_line_width;
use super::super::types::LineStyle;
use crate::output::LineSeries;

/// Render a band (upper, optional middle, lower) with fill between
/// upper and lower.
pub fn render_band(
    canvas: &mut dyn Canvas,
    upper: &LineSeries,
    middle: Option<&LineSeries>,
    lower: &LineSeries,
    fill_opacity: f32,
    view: &dyn ChartView,
) {
    if upper.points.is_empty() || lower.points.is_empty() {
        return;
    }

    if fill_opacity > 0.0 {
        // Soft glow: slightly expanded fill at lower alpha
        render_band_fill(canvas, upper, lower, fill_opacity * 0.3, view, 1.0);
        // Main fill
        render_band_fill(canvas, upper, lower, fill_opacity, view, 0.0);
    }

    // Draw the lines on top of the fill
    render_band_line(canvas, upper, view);
    if let Some(mid) = middle {
        render_band_line(canvas, mid, view);
    }
    render_band_line(canvas, lower, view);
}

fn render_band_line(
    canvas: &mut dyn Canvas,
    series: &LineSeries,
    view: &dyn ChartView,
) {
    if series.points.len() < 2 {
        return;
    }

    let width = effective_line_width(series.width, view.scaling()).max(0.5);
    let style = LineStyle::from(&series.style);

    let points: Vec<(f32, f32)> = series
        .points
        .iter()
        .map(|&(x_val, y_val)| {
            (view.interval_to_x(x_val), view.value_to_y(y_val))
        })
        .collect();

    canvas.stroke_polyline(&points, series.color, width, style);
}

fn render_band_fill(
    canvas: &mut dyn Canvas,
    upper: &LineSeries,
    lower: &LineSeries,
    fill_opacity: f32,
    view: &dyn ChartView,
    offset: f32,
) {
    let fill_color = upper.color.scale_alpha(fill_opacity);

    // Upper points left-to-right (offset upward = subtract from Y)
    let mut polygon: Vec<(f32, f32)> = upper
        .points
        .iter()
        .map(|&(x_val, y_val)| {
            (
                view.interval_to_x(x_val),
                view.value_to_y(y_val) - offset,
            )
        })
        .collect();

    // Lower points right-to-left (offset downward = add to Y)
    let lower_reversed: Vec<(f32, f32)> = lower
        .points
        .iter()
        .rev()
        .map(|&(x_val, y_val)| {
            (
                view.interval_to_x(x_val),
                view.value_to_y(y_val) + offset,
            )
        })
        .collect();

    polygon.extend(lower_reversed);

    if polygon.len() >= 3 {
        canvas.fill_polygon(&polygon, fill_color);
    }
}
