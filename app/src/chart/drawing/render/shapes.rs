//! Shape drawing rendering: Rectangle, Ellipse.

use super::super::Drawing;
use super::{DrawContext, draw_label, draw_rect_with_fill};
use crate::drawing::DrawingTool;
use iced::widget::canvas::{Frame, Path};
use iced::{Color, Point};

pub fn draw(frame: &mut Frame, ctx: &DrawContext<'_>, drawing: &Drawing, pts: &[Point]) {
    match drawing.tool {
        DrawingTool::Rectangle => draw_rectangle(frame, ctx, drawing, pts),
        DrawingTool::Ellipse => draw_ellipse(frame, ctx, drawing, pts),
        DrawingTool::AiContext => draw_ai_context(frame, ctx, drawing, pts),
        _ => {}
    }
}

fn draw_rectangle(frame: &mut Frame, ctx: &DrawContext<'_>, drawing: &Drawing, pts: &[Point]) {
    if pts.len() >= 2 {
        draw_rect_with_fill(frame, pts, drawing, ctx.stroke, ctx.alpha);
    }
}

fn draw_ellipse(frame: &mut Frame, ctx: &DrawContext<'_>, drawing: &Drawing, pts: &[Point]) {
    if pts.len() < 2 {
        return;
    }
    let cx = pts[0].x;
    let cy = pts[0].y;
    let rx = (pts[1].x - cx).abs().max(1.0);
    let ry = (pts[1].y - cy).abs().max(1.0);

    let ellipse = Path::new(|builder| {
        builder.move_to(Point::new(cx + rx, cy));
        let steps = 64;
        for i in 1..=steps {
            let angle = 2.0 * std::f32::consts::PI * (i as f32 / steps as f32);
            builder.line_to(Point::new(cx + rx * angle.cos(), cy + ry * angle.sin()));
        }
        builder.close();
    });

    if let Some(fill_color) = drawing.fill_color() {
        frame.fill(
            &ellipse,
            fill_color.scale_alpha(drawing.style.fill_opacity * ctx.alpha),
        );
    }

    frame.stroke(&ellipse, ctx.stroke);
}

fn draw_ai_context(frame: &mut Frame, ctx: &DrawContext<'_>, drawing: &Drawing, pts: &[Point]) {
    if pts.len() < 2 {
        return;
    }

    // Draw the rectangle with fill (reuse shared helper)
    draw_rect_with_fill(frame, pts, drawing, ctx.stroke, ctx.alpha);

    // Draw "AI" badge in top-right corner
    let max_x = pts[0].x.max(pts[1].x);
    let min_y = pts[0].y.min(pts[1].y);
    let badge_pos = Point::new(max_x - 18.0, min_y + 2.0);
    let badge_color = Color {
        a: ctx.alpha * 0.9,
        ..ctx.stroke_color
    };
    draw_label(frame, "AI", badge_pos, badge_color);
}
