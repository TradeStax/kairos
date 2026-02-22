//! Line-based drawing rendering: Line, Ray, ExtendedLine, Arrow,
//! HorizontalLine, VerticalLine.

use super::{DrawContext, draw_drawing_label, extend_to_bounds};
use super::super::Drawing;
use crate::chart::core::tokens;
use data::DrawingTool;
use iced::widget::canvas::{Frame, Path};
use iced::Point;

pub fn draw(frame: &mut Frame, ctx: &DrawContext<'_>, drawing: &Drawing, pts: &[Point]) {
    match drawing.tool {
        DrawingTool::HorizontalLine => draw_horizontal(frame, ctx, drawing, pts),
        DrawingTool::VerticalLine => draw_vertical(frame, ctx, drawing, pts),
        DrawingTool::Line => draw_line(frame, ctx, drawing, pts),
        DrawingTool::Ray => draw_ray(frame, ctx, drawing, pts),
        DrawingTool::ExtendedLine => draw_extended(frame, ctx, drawing, pts),
        DrawingTool::Arrow => draw_arrow(frame, ctx, drawing, pts),
        _ => {}
    }
}

fn draw_horizontal(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    let Some(p) = pts.first() else { return };
    let start = Point::new(0.0, p.y);
    let end = Point::new(ctx.bounds.width, p.y);
    frame.stroke(&Path::line(start, end), ctx.stroke);
    draw_drawing_label(frame, drawing, &[start, end], ctx.bounds, ctx.stroke_color, false);
}

fn draw_vertical(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    let Some(p) = pts.first() else { return };
    let start = Point::new(p.x, 0.0);
    let end = Point::new(p.x, ctx.bounds.height);
    frame.stroke(&Path::line(start, end), ctx.stroke);
    draw_drawing_label(frame, drawing, &[start, end], ctx.bounds, ctx.stroke_color, true);
}

fn draw_line(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    if pts.len() >= 2 {
        frame.stroke(&Path::line(pts[0], pts[1]), ctx.stroke);
        draw_drawing_label(frame, drawing, &pts[..2], ctx.bounds, ctx.stroke_color, false);
    }
}

fn draw_ray(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    if pts.len() >= 2 {
        let forward = extend_to_bounds(pts[0], pts[1], ctx.bounds);
        frame.stroke(&Path::line(pts[0], forward), ctx.stroke);
        draw_drawing_label(frame, drawing, &[pts[0], forward], ctx.bounds, ctx.stroke_color, false);
    } else if let Some(p) = pts.first() {
        let end = Point::new(ctx.bounds.width, p.y);
        frame.stroke(&Path::line(*p, end), ctx.stroke);
        draw_drawing_label(frame, drawing, &[*p, end], ctx.bounds, ctx.stroke_color, false);
    }
}

fn draw_extended(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    if pts.len() >= 2 {
        let back = extend_to_bounds(pts[1], pts[0], ctx.bounds);
        let forward = extend_to_bounds(pts[0], pts[1], ctx.bounds);
        frame.stroke(&Path::line(back, forward), ctx.stroke);
        draw_drawing_label(
            frame, drawing, &[back, forward], ctx.bounds, ctx.stroke_color, false,
        );
    }
}

fn draw_arrow(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    _drawing: &Drawing,
    pts: &[Point],
) {
    if pts.len() < 2 {
        return;
    }
    let start = pts[0];
    let end = pts[1];

    frame.stroke(&Path::line(start, end), ctx.stroke);

    // Arrowhead
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len > 1.0 {
        let arrow_len = tokens::ruler::ARROW_LENGTH;
        let arrow_width = tokens::ruler::ARROW_WIDTH;
        let ux = dx / len;
        let uy = dy / len;
        let left = Point::new(
            end.x - ux * arrow_len + uy * arrow_width,
            end.y - uy * arrow_len - ux * arrow_width,
        );
        let right = Point::new(
            end.x - ux * arrow_len - uy * arrow_width,
            end.y - uy * arrow_len + ux * arrow_width,
        );
        let arrow = Path::new(|builder| {
            builder.move_to(end);
            builder.line_to(left);
            builder.line_to(right);
            builder.close();
        });
        frame.fill(&arrow, ctx.stroke_color);
    }
}
