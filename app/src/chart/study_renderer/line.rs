//! Line series renderer
//!
//! Renders `LineSeries` as connected polylines on the chart canvas.

use super::{value_range, value_to_panel_y};
use crate::chart::ViewState;
use exchange::util::Price;
use iced::widget::canvas::{Frame, LineDash, Path, Stroke};
use iced::{Color, Point, Size};
use study::StudyPlacement;
use study::output::LineSeries;

/// Render one or more line series.
pub fn render_lines(
    frame: &mut Frame,
    lines: &[LineSeries],
    state: &ViewState,
    bounds: Size,
    placement: StudyPlacement,
) {
    if lines.is_empty() {
        return;
    }

    // For panel placement, compute a shared Y range across all series
    let panel_range = if placement == StudyPlacement::Panel {
        let all_values = lines.iter().flat_map(|s| s.points.iter().map(|(_, v)| *v));
        value_range(all_values)
    } else {
        None
    };

    for series in lines {
        render_single_line(frame, series, state, bounds, placement, panel_range);
    }
}

fn render_single_line(
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

    let color: Color = crate::style::theme_bridge::rgba_to_iced_color(series.color);
    // Divide width by scaling so lines maintain a consistent
    // screen-pixel thickness regardless of zoom level.
    let effective_width = if state.scaling > f32::EPSILON {
        series.width / state.scaling
    } else {
        series.width
    };
    let stroke = Stroke {
        width: effective_width,
        line_dash: line_dash_for_style(&series.style),
        ..Stroke::default()
    };
    let stroke = Stroke::with_color(stroke, color);

    let mut prev: Option<Point> = None;

    for &(x_val, y_val) in &series.points {
        let sx = state.interval_to_x(x_val);
        let sy = match placement {
            StudyPlacement::Overlay | StudyPlacement::Background => {
                state.price_to_y(Price::from_f32_lossy(y_val))
            }
            StudyPlacement::Panel => {
                if let Some((min, max)) = panel_range {
                    value_to_panel_y(y_val, min, max, bounds.height)
                } else {
                    bounds.height
                }
            }
        };

        let point = Point::new(sx, sy);
        if let Some(p) = prev {
            frame.stroke(&Path::line(p, point), stroke);
        }
        prev = Some(point);
    }
}

/// Convert a `LineStyleValue` to an iced `LineDash`.
fn line_dash_for_style(style: &study::config::LineStyleValue) -> LineDash<'static> {
    match style {
        study::config::LineStyleValue::Solid => LineDash::default(),
        study::config::LineStyleValue::Dashed => LineDash {
            segments: &[6.0, 4.0],
            offset: 0,
        },
        study::config::LineStyleValue::Dotted => LineDash {
            segments: &[2.0, 3.0],
            offset: 0,
        },
    }
}
