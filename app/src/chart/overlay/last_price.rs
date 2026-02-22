//! Last Price Line Overlay
//!
//! Draws a horizontal dashed line at the last traded price with a
//! floating price label box at the right edge of the chart area.

use crate::chart::core::tokens;
use crate::chart::core::ViewState;
use crate::components::primitives::AZERET_MONO;
use data::util::count_decimals;
use iced::theme::palette::Extended;
use iced::widget::canvas::{Frame, LineDash, Path, Stroke, Text};
use iced::{Color, Point, Rectangle, Size};

/// Draw the last price line on the chart with a price label box
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

        // Draw price label box at the right edge
        let precision = count_decimals(state.ticker_info.tick_size);
        let price_text = format!("{:.prec$}", last_price.to_f32(), prec = precision);

        let text_size = crate::style::tokens::text::SMALL;
        let char_width = text_size * 0.7;
        let label_width =
            price_text.len() as f32 * char_width + tokens::last_price::LABEL_PADDING_X * 2.0;
        let label_height = text_size + tokens::last_price::LABEL_PADDING_Y * 2.0;

        let label_x = region.x + region.width
            - label_width
            - tokens::last_price::LABEL_MARGIN_RIGHT;
        let label_y = y_pos - label_height / 2.0;

        // Fill label background with the line color
        frame.fill_rectangle(
            Point::new(label_x, label_y),
            Size::new(label_width, label_height),
            line_color,
        );

        // Determine contrasting text color
        let text_color = contrast_text_color(line_color);

        frame.fill_text(Text {
            content: price_text,
            position: Point::new(
                label_x + tokens::last_price::LABEL_PADDING_X,
                label_y + tokens::last_price::LABEL_PADDING_Y,
            ),
            size: iced::Pixels(text_size),
            color: text_color,
            font: AZERET_MONO,
            ..Text::default()
        });
    }
}

/// Choose black or white text based on the background color luminance.
fn contrast_text_color(bg: Color) -> Color {
    let luminance = 0.299 * bg.r + 0.587 * bg.g + 0.114 * bg.b;
    if luminance > 0.5 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}
