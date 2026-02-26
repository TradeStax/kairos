//! Fibonacci drawing rendering: FibRetracement, FibExtension.

use super::super::Drawing;
use super::{DrawContext, create_stroke, draw_label};
use crate::drawing::{DrawingTool, FibonacciConfig, LineStyle};
use iced::widget::canvas::{Frame, Path};
use iced::{Color, Point, Size};

pub fn draw(frame: &mut Frame, ctx: &DrawContext<'_>, drawing: &Drawing, pts: &[Point]) {
    match drawing.tool {
        DrawingTool::FibRetracement => draw_retracement(frame, ctx, drawing, pts),
        DrawingTool::FibExtension => draw_extension(frame, ctx, drawing, pts),
        _ => {}
    }
}

fn draw_retracement(frame: &mut Frame, ctx: &DrawContext<'_>, drawing: &Drawing, pts: &[Point]) {
    if pts.len() < 2 {
        return;
    }
    let config = drawing
        .style
        .fibonacci
        .as_ref()
        .cloned()
        .unwrap_or_default();
    draw_fib_levels(
        frame,
        &pts[0],
        &pts[1],
        &config,
        ctx.stroke_width,
        drawing.style.line_style,
        ctx.bounds,
    );
}

fn draw_extension(frame: &mut Frame, ctx: &DrawContext<'_>, drawing: &Drawing, pts: &[Point]) {
    if pts.len() < 3 {
        return;
    }
    let config = drawing
        .style
        .fibonacci
        .as_ref()
        .cloned()
        .unwrap_or_default();

    let y_range = pts[1].y - pts[0].y;
    let min_x = pts.iter().map(|p| p.x).fold(f32::MAX, f32::min);
    let max_x = pts.iter().map(|p| p.x).fold(f32::MIN, f32::max);

    for level in &config.levels {
        if !level.visible {
            continue;
        }
        let level_y = pts[2].y + y_range * level.ratio as f32;
        let level_color: Color = crate::style::theme::rgba_to_iced_color(level.color);
        let level_stroke = create_stroke(level_color, ctx.stroke_width, drawing.style.line_style);

        let (lx, rx) = if config.extend_lines {
            (0.0, ctx.bounds.width)
        } else {
            (min_x, max_x)
        };

        let path = Path::line(Point::new(lx, level_y), Point::new(rx, level_y));
        frame.stroke(&path, level_stroke);

        if config.show_percentages {
            draw_label(
                frame,
                &level.label,
                Point::new(rx + 4.0, level_y - 8.0),
                level_color,
            );
        }
    }

    // Draw anchor lines
    let anchor_stroke = create_stroke(ctx.stroke_color.scale_alpha(0.4), 1.0, LineStyle::Dashed);
    frame.stroke(&Path::line(pts[0], pts[1]), anchor_stroke);
    frame.stroke(&Path::line(pts[1], pts[2]), anchor_stroke);
}

/// Draw fibonacci retracement levels between two points.
fn draw_fib_levels(
    frame: &mut Frame,
    p1: &Point,
    p2: &Point,
    config: &FibonacciConfig,
    stroke_width: f32,
    line_style: LineStyle,
    bounds: Size,
) {
    let y_range = p2.y - p1.y;
    let min_x = p1.x.min(p2.x);
    let max_x = p1.x.max(p2.x);

    for level in &config.levels {
        if !level.visible {
            continue;
        }

        let level_y = p1.y + y_range * level.ratio as f32;
        let level_color: Color = crate::style::theme::rgba_to_iced_color(level.color);
        let level_stroke = create_stroke(level_color, stroke_width, line_style);

        let (lx, rx) = if config.extend_lines {
            (0.0, bounds.width)
        } else {
            (min_x, max_x)
        };

        let path = Path::line(Point::new(lx, level_y), Point::new(rx, level_y));
        frame.stroke(&path, level_stroke);

        if config.show_percentages {
            draw_label(
                frame,
                &level.label,
                Point::new(rx + 4.0, level_y - 8.0),
                level_color,
            );
        }
    }

    // Draw fill between levels using first level's color with low opacity
    let fill_color = if let Some(level) = config.levels.first() {
        let c: Color = crate::style::theme::rgba_to_iced_color(level.color);
        c.scale_alpha(0.05)
    } else {
        Color::from_rgba(0.5, 0.5, 0.5, 0.05)
    };
    let fill_path = Path::rectangle(
        Point::new(min_x, p1.y.min(p2.y)),
        Size::new(max_x - min_x, (p2.y - p1.y).abs()),
    );
    frame.fill(&fill_path, fill_color);
}
