//! Last Price Line Overlay
//!
//! Draws a horizontal dashed line at the last traded price.

use crate::chart::core::ViewState;
use iced::theme::palette::Extended;
use iced::widget::canvas::{Frame, LineDash, Path, Stroke};
use iced::{Point, Rectangle};

/// Draw the last price line on the chart
pub fn draw_last_price_line(
    state: &ViewState,
    frame: &mut Frame,
    palette: &Extended,
    region: Rectangle,
) {
    if let Some(price) = &state.last_price {
        let (last_price, line_color) = price.get_with_color(palette);
        let y_pos = state.price_to_y(last_price);

        let marker_line = Stroke::with_color(
            Stroke {
                width: 1.0,
                line_dash: LineDash {
                    segments: &[2.0, 2.0],
                    offset: 4,
                },
                ..Default::default()
            },
            line_color.scale_alpha(0.5),
        );

        frame.stroke(
            &Path::line(
                Point::new(0.0, y_pos),
                Point::new(region.x + region.width, y_pos),
            ),
            marker_line,
        );
    }
}
