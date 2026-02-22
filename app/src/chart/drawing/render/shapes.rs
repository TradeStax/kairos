//! Shape drawing rendering: Rectangle, Ellipse, PriceRange, DateRange.

use super::{DrawContext, draw_label, draw_rect_with_fill, format_duration};
use super::super::Drawing;
use data::DrawingTool;
use iced::widget::canvas::{Frame, Path};
use iced::Point;

pub fn draw(frame: &mut Frame, ctx: &DrawContext<'_>, drawing: &Drawing, pts: &[Point]) {
    match drawing.tool {
        DrawingTool::Rectangle => draw_rectangle(frame, ctx, drawing, pts),
        DrawingTool::PriceRange => draw_price_range(frame, ctx, drawing, pts),
        DrawingTool::DateRange => draw_date_range(frame, ctx, drawing, pts),
        DrawingTool::Ellipse => draw_ellipse(frame, ctx, drawing, pts),
        _ => {}
    }
}

fn draw_rectangle(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    if pts.len() >= 2 {
        draw_rect_with_fill(frame, pts, drawing, ctx.stroke, ctx.alpha);
    }
}

fn draw_price_range(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    if pts.len() < 2 {
        return;
    }
    draw_rect_with_fill(frame, pts, drawing, ctx.stroke, ctx.alpha);

    let p1 = &drawing.points[0];
    let p2 = &drawing.points[1];
    let delta = p2.price.units() - p1.price.units();
    let label = format!(
        "{}{:.2}",
        if delta >= 0 { "+" } else { "" },
        data::Price::from_units(delta).to_f64()
    );
    let mid = Point::new(
        (pts[0].x + pts[1].x) / 2.0,
        (pts[0].y + pts[1].y) / 2.0,
    );
    draw_label(frame, &label, mid, ctx.stroke_color);
}

fn draw_date_range(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    if pts.len() < 2 {
        return;
    }
    draw_rect_with_fill(frame, pts, drawing, ctx.stroke, ctx.alpha);

    let p1 = &drawing.points[0];
    let p2 = &drawing.points[1];
    let t1 = p1.time.min(p2.time);
    let t2 = p1.time.max(p2.time);
    let label = format_duration(t2 - t1);
    let mid = Point::new(
        (pts[0].x + pts[1].x) / 2.0,
        (pts[0].y + pts[1].y) / 2.0,
    );
    draw_label(frame, &label, mid, ctx.stroke_color);
}

fn draw_ellipse(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
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
