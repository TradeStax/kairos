//! Histogram renderer
//!
//! Renders MACD-style histogram bars with a centered baseline at 0.
//! Positive bars go up, negative bars go down. Colors come from each
//! `HistogramBar`.

use super::coord;
use crate::chart::ViewState;
use exchange::util::Price;
use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Color, Point, Size};
use study::StudyPlacement;
use study::output::HistogramBar;

/// Render histogram bars.
pub fn render_histogram(
    frame: &mut Frame,
    bars: &[HistogramBar],
    state: &ViewState,
    bounds: Size,
    placement: StudyPlacement,
) {
    if bars.is_empty() {
        return;
    }

    // For panel placement, compute value range including zero
    let panel_range = if placement == StudyPlacement::Panel {
        let range = coord::value_range(bars.iter().map(|b| b.value));
        range.map(|(min, max)| (min.min(0.0), max.max(0.0)))
    } else {
        None
    };

    let bar_width = state.cell_width * 0.6;

    for bar in bars {
        let sx = state.interval_to_x(bar.x);
        let left = sx - bar_width / 2.0;
        let color: Color = crate::style::theme_bridge::rgba_to_iced_color(bar.color);

        match placement {
            StudyPlacement::CandleReplace => return,
            StudyPlacement::Overlay | StudyPlacement::Background => {
                let y_val = state.price_to_y(Price::from_f32(bar.value));
                let y_zero = state.price_to_y(Price::from_f32(0.0));

                let (top, height) = if bar.value >= 0.0 {
                    (y_val, y_zero - y_val)
                } else {
                    (y_zero, y_val - y_zero)
                };

                if height > 0.0 {
                    frame.fill_rectangle(
                        Point::new(left, top),
                        Size::new(bar_width, height),
                        color,
                    );
                }
            }
            StudyPlacement::Panel => {
                if let Some((min, max)) = panel_range {
                    let y_val = coord::value_to_panel_y(bar.value, min, max, bounds.height);
                    let y_zero = coord::value_to_panel_y(0.0, min, max, bounds.height);

                    let (top, height) = if bar.value >= 0.0 {
                        (y_val, y_zero - y_val)
                    } else {
                        (y_zero, y_val - y_zero)
                    };

                    if height > 0.0 {
                        frame.fill_rectangle(
                            Point::new(left, top),
                            Size::new(bar_width, height),
                            color,
                        );
                    }
                }
            }
        }
    }

    // Draw a subtle zero-line baseline
    draw_zero_line(frame, state, bounds, placement, panel_range);
}

/// Draw a subtle 1px line at y=0 across the full width.
fn draw_zero_line(
    frame: &mut Frame,
    state: &ViewState,
    bounds: Size,
    placement: StudyPlacement,
    panel_range: Option<(f32, f32)>,
) {
    let y_zero = match placement {
        StudyPlacement::Overlay | StudyPlacement::Background => {
            state.price_to_y(Price::from_f32(0.0))
        }
        StudyPlacement::Panel => {
            if let Some((min, max)) = panel_range {
                coord::value_to_panel_y(0.0, min, max, bounds.height)
            } else {
                return;
            }
        }
        StudyPlacement::CandleReplace => return,
    };

    // Only draw if the zero-line is within visible bounds
    if y_zero < 0.0 || y_zero > bounds.height {
        return;
    }

    let stroke = Stroke::with_color(
        Stroke {
            width: 1.0,
            ..Stroke::default()
        },
        Color::from_rgba(1.0, 1.0, 1.0, 0.2),
    );
    frame.stroke(
        &Path::line(
            Point::new(0.0, y_zero),
            Point::new(bounds.width, y_zero),
        ),
        stroke,
    );
}
