//! Study tooltip overlay rendering.
//!
//! Renders a tooltip card near the cursor when hovering over an
//! interactive study region (markers, zones, levels).

use crate::chart::core::HoveredStudyRegion;
use crate::components::primitives::AZERET_MONO;
use iced::widget::canvas::{Frame, Stroke, Text};
use iced::{Color, Point, Size};

/// Draw a multi-line tooltip near the hovered study region.
pub fn draw_study_tooltip(
    frame: &mut Frame,
    hovered: &HoveredStudyRegion,
    bounds: Size,
) {
    let text = &hovered.tooltip;
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return;
    }

    let font_size = 11.0;
    let padding = 6.0;
    let line_height = font_size + 3.0;
    let tooltip_height = lines.len() as f32 * line_height + padding * 2.0;
    let max_chars = lines.iter().map(|l: &&str| l.len()).max().unwrap_or(0);
    let tooltip_width = max_chars as f32 * 6.2 + padding * 2.0;

    // Position near cursor, clamped to chart bounds
    let mut x = hovered.screen_pos.x + 14.0;
    let mut y = hovered.screen_pos.y - tooltip_height - 6.0;
    if x + tooltip_width > bounds.width {
        x = hovered.screen_pos.x - tooltip_width - 14.0;
    }
    if y < 0.0 {
        y = hovered.screen_pos.y + 20.0;
    }
    if x < 0.0 {
        x = 4.0;
    }

    let bg = Color {
        r: 0.10,
        g: 0.10,
        b: 0.12,
        a: 0.93,
    };
    frame.fill_rectangle(
        Point::new(x, y),
        Size::new(tooltip_width, tooltip_height),
        bg,
    );

    let border_color = Color {
        r: 0.30,
        g: 0.30,
        b: 0.35,
        a: 0.7,
    };
    frame.stroke_rectangle(
        Point::new(x, y),
        Size::new(tooltip_width, tooltip_height),
        Stroke::default()
            .with_color(border_color)
            .with_width(1.0),
    );

    let text_color = Color {
        r: 0.88,
        g: 0.88,
        b: 0.88,
        a: 1.0,
    };
    for (i, line) in lines.into_iter().enumerate() {
        frame.fill_text(Text {
            content: String::from(line),
            position: Point::new(
                x + padding,
                y + padding + i as f32 * line_height,
            ),
            color: text_color,
            size: iced::Pixels(font_size),
            font: AZERET_MONO,
            ..Text::default()
        });
    }
}
