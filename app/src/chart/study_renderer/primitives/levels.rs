//! Levels renderer
//!
//! Renders horizontal price level lines (Fibonacci, Support/Resistance).
//! Each level is drawn as a full-width horizontal line at the given price,
//! or as a rightward ray when `start_x` is set.

use super::super::coord::line_dash_for_style;
use crate::chart::ViewState;
use crate::components::primitives::AZERET_MONO;
use crate::style::tokens::{spacing, text};
use data::Price;
use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::{Color, Point, Size};
use study::output::PriceLevel;

/// Width fractions of zone_half_width for each strip (outermost
/// first).
const ZONE_STRIP_WIDTHS: [f32; 3] = [1.0, 0.65, 0.3];
/// Alpha multipliers for each strip (outermost = most transparent).
const ZONE_STRIP_ALPHAS: [f32; 3] = [0.04, 0.07, 0.12];

/// Render horizontal price levels.
pub fn render_levels(frame: &mut Frame, levels: &[PriceLevel], state: &ViewState, bounds: Size) {
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

        let right = match level.end_x {
            Some(x) => state.interval_to_x(x),
            None => bounds.width * 2.0,
        };

        // Cull: bounded zone ends before the visible area.
        if right < 0.0 {
            continue;
        }

        let color: Color = crate::style::theme::rgba_to_iced_color(level.color);
        let color = color.scale_alpha(level.opacity);

        // Fill above if configured
        if let Some((fill_color, fill_opacity)) = &level.fill_above {
            let fc: Color = crate::style::theme::rgba_to_iced_color(*fill_color);
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
            let fc: Color = crate::style::theme::rgba_to_iced_color(*fill_color);
            let fill = fc.scale_alpha(*fill_opacity);
            frame.fill_rectangle(
                Point::new(left, y),
                Size::new(right - left, bounds.height * 2.0),
                fill,
            );
        }

        // Zone rendering: concentric shaded strips
        if let Some(zone_hw) = level.zone_half_width {
            let y_above = state.price_to_y(Price::from_f32((level.price + zone_hw) as f32));
            let full_half = (y - y_above).abs();

            for i in 0..ZONE_STRIP_WIDTHS.len() {
                let strip_half = full_half * ZONE_STRIP_WIDTHS[i];
                // `color` already includes level.opacity — apply
                // only the per-strip alpha to avoid double scaling.
                let fill_color = color.scale_alpha(ZONE_STRIP_ALPHAS[i]);
                frame.fill_rectangle(
                    Point::new(left, y - strip_half),
                    Size::new(right - left, strip_half * 2.0),
                    fill_color,
                );
            }
        }

        // When zone is active the shaded area carries the visual
        // weight — draw a very thin, semi-transparent center line.
        let (line_width, line_color) = if level.zone_half_width.is_some() {
            (0.5_f32, color.scale_alpha(0.35))
        } else {
            (level.width, color)
        };
        let dash = line_dash_for_style(&level.style);
        let stroke = Stroke::with_color(
            Stroke {
                width: line_width,
                line_dash: dash,
                ..Stroke::default()
            },
            line_color,
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
                position: Point::new(label_x, y - spacing::LG),
                size: iced::Pixels(text::TINY),
                color,
                font: AZERET_MONO,
                ..Text::default()
            });
        }
    }
}
