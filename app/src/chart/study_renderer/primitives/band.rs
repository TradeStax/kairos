//! Band renderer
//!
//! Renders Bollinger-style bands: upper line, optional middle line, lower line,
//! with a semi-transparent fill between upper and lower.

use super::super::coord;
use crate::chart::ViewState;
use exchange::util::Price;
use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Color, Point, Size};
use study::StudyPlacement;
use study::output::LineSeries;

/// Render a band (upper, optional middle, lower) with fill between upper and lower.
pub fn render_band(
    frame: &mut Frame,
    upper: &LineSeries,
    middle: Option<&LineSeries>,
    lower: &LineSeries,
    fill_opacity: f32,
    state: &ViewState,
    bounds: Size,
    placement: StudyPlacement,
) {
    if upper.points.is_empty() || lower.points.is_empty() {
        return;
    }

    // For panel placement, compute shared Y range across all band series
    let panel_range = if placement == StudyPlacement::Panel {
        let all_values = upper
            .points
            .iter()
            .chain(lower.points.iter())
            .map(|(_, v)| *v);
        coord::value_range(all_values)
    } else {
        None
    };

    // Fill between upper and lower
    if fill_opacity > 0.0 {
        // Soft glow: slightly expanded fill at lower alpha
        render_band_fill_with_offset(
            frame,
            upper,
            lower,
            fill_opacity * 0.3,
            state,
            bounds,
            placement,
            panel_range,
            1.0, // 1px expansion beyond band edges
        );
        // Main fill
        render_band_fill_with_offset(
            frame,
            upper,
            lower,
            fill_opacity,
            state,
            bounds,
            placement,
            panel_range,
            0.0,
        );
    }

    // Draw the lines on top of the fill
    render_band_line(frame, upper, state, bounds, placement, panel_range);
    if let Some(mid) = middle {
        render_band_line(frame, mid, state, bounds, placement, panel_range);
    }
    render_band_line(frame, lower, state, bounds, placement, panel_range);
}

fn render_band_line(
    frame: &mut Frame,
    series: &LineSeries,
    state: &ViewState,
    bounds: Size,
    placement: StudyPlacement,
    panel_range: Option<(f32, f32)>,
) {
    if series.points.len() < 2 {
        return;
    }

    let color: Color = crate::style::theme::rgba_to_iced_color(series.color);
    let stroke = Stroke::with_color(
        Stroke {
            width: series.width,
            line_dash: coord::line_dash_for_style(&series.style),
            ..Stroke::default()
        },
        color,
    );

    let mut prev: Option<Point> = None;
    for &(x_val, y_val) in &series.points {
        let sx = state.interval_to_x(x_val);
        let sy = to_y(y_val, state, bounds, placement, panel_range);

        let point = Point::new(sx, sy);
        if let Some(p) = prev {
            frame.stroke(&Path::line(p, point), stroke);
        }
        prev = Some(point);
    }
}

fn render_band_fill_with_offset(
    frame: &mut Frame,
    upper: &LineSeries,
    lower: &LineSeries,
    fill_opacity: f32,
    state: &ViewState,
    bounds: Size,
    placement: StudyPlacement,
    panel_range: Option<(f32, f32)>,
    offset: f32,
) {
    // Build upper and lower screen points with matching x positions.
    // Use the upper series color for the fill.
    let fill_color: Color = crate::style::theme::rgba_to_iced_color(upper.color);
    let fill_color = fill_color.scale_alpha(fill_opacity);

    // Collect upper points (offset upward = subtract from screen Y)
    let upper_pts: Vec<Point> = upper
        .points
        .iter()
        .map(|&(x_val, y_val)| {
            let sx = state.interval_to_x(x_val);
            let sy = to_y(y_val, state, bounds, placement, panel_range) - offset;
            Point::new(sx, sy)
        })
        .collect();

    // Collect lower points (reversed for closing the polygon,
    // offset downward = add to screen Y)
    let lower_pts: Vec<Point> = lower
        .points
        .iter()
        .rev()
        .map(|&(x_val, y_val)| {
            let sx = state.interval_to_x(x_val);
            let sy = to_y(y_val, state, bounds, placement, panel_range) + offset;
            Point::new(sx, sy)
        })
        .collect();

    debug_assert_eq!(
        upper.points.len(),
        lower.points.len(),
        "Band upper/lower series must have equal point count for correct fill polygon"
    );

    if upper_pts.is_empty() || lower_pts.is_empty() {
        return;
    }

    // Build a closed polygon: upper left-to-right, then lower right-to-left
    let path = Path::new(|builder| {
        let first = upper_pts[0];
        builder.move_to(first);
        for &pt in &upper_pts[1..] {
            builder.line_to(pt);
        }
        for &pt in &lower_pts {
            builder.line_to(pt);
        }
        builder.close();
    });

    frame.fill(&path, fill_color);
}

fn to_y(
    value: f32,
    state: &ViewState,
    bounds: Size,
    placement: StudyPlacement,
    panel_range: Option<(f32, f32)>,
) -> f32 {
    match placement {
        StudyPlacement::Overlay
        | StudyPlacement::Background
        | StudyPlacement::CandleReplace
        | StudyPlacement::SidePanel => {
            state.price_to_y(Price::from_f32(value))
        }
        StudyPlacement::Panel => {
            if let Some((min, max)) = panel_range {
                coord::value_to_panel_y(value, min, max, bounds.height)
            } else {
                bounds.height
            }
        }
    }
}
