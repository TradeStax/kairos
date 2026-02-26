//! Levels renderer
//!
//! Renders horizontal price level lines (Fibonacci, Support/Resistance).
//! Each level is drawn as a full-width horizontal line at the given price,
//! or as a rightward ray when `start_x` is set.

use super::super::coord::line_dash_for_style;
use crate::chart::ViewState;
use crate::components::primitives::AZERET_MONO;
use data::Price;
use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::{Color, Point, Size};
use study::output::PriceLevel;

/// Render horizontal price levels.
pub fn render_levels(
    frame: &mut Frame,
    levels: &[PriceLevel],
    state: &ViewState,
    bounds: Size,
) {
    for level in levels {
        let y = state.price_to_y(Price::from_f32(level.price as f32));

        // When start_x is set, draw a ray from the anchor rightward.
        // Otherwise draw a full-width line.
        let left = match level.start_x {
            Some(x) => state.interval_to_x(x),
            None => -bounds.width * 2.0,
        };

        // Cull: ray starts past the right edge — entirely off-screen.
        if left > bounds.width {
            continue;
        }

        let right = bounds.width * 2.0;

        let color: Color =
            crate::style::theme::rgba_to_iced_color(level.color);
        let color = color.scale_alpha(level.opacity);

        // Fill above if configured
        if let Some((fill_color, fill_opacity)) = &level.fill_above {
            let fc: Color =
                crate::style::theme::rgba_to_iced_color(*fill_color);
            let fill = fc.scale_alpha(*fill_opacity);
            let top_y = -bounds.height;
            let fill_height = y - top_y;
            if fill_height > 0.0 {
                frame.fill_rectangle(
                    Point::new(left, top_y),
                    Size::new(right - left, fill_height),
                    fill,
                );
            }
        }

        // Fill below if configured
        if let Some((fill_color, fill_opacity)) = &level.fill_below {
            let fc: Color =
                crate::style::theme::rgba_to_iced_color(*fill_color);
            let fill = fc.scale_alpha(*fill_opacity);
            frame.fill_rectangle(
                Point::new(left, y),
                Size::new(right - left, bounds.height * 2.0),
                fill,
            );
        }

        // Draw the horizontal line
        let dash = line_dash_for_style(&level.style);
        let stroke = Stroke::with_color(
            Stroke {
                width: 1.0,
                line_dash: dash,
                ..Stroke::default()
            },
            color,
        );

        frame.stroke(
            &Path::line(Point::new(left, y), Point::new(right, y)),
            stroke,
        );

        // Draw label if enabled
        if level.show_label && !level.label.is_empty() {
            let label_x = left.max(4.0);
            frame.fill_text(Text {
                content: level.label.clone(),
                position: Point::new(label_x, y - 12.0),
                size: iced::Pixels(10.0),
                color,
                font: AZERET_MONO,
                ..Text::default()
            });
        }
    }
}
