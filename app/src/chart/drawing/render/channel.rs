//! Parallel channel drawing rendering.

use super::{DrawContext, create_stroke};
use super::super::Drawing;
use data::LineStyle;
use iced::widget::canvas::{Frame, Path};
use iced::Point;

pub fn draw(frame: &mut Frame, ctx: &DrawContext<'_>, drawing: &Drawing, pts: &[Point]) {
    if pts.len() < 3 {
        return;
    }

    // Line 1: points[0] to points[1]
    frame.stroke(&Path::line(pts[0], pts[1]), ctx.stroke);

    // Line 2: parallel through points[2]
    let dx = pts[1].x - pts[0].x;
    let dy = pts[1].y - pts[0].y;
    let p2_end = Point::new(pts[2].x + dx, pts[2].y + dy);
    frame.stroke(&Path::line(pts[2], p2_end), ctx.stroke);

    // Center line (dashed)
    let center_start = Point::new(
        (pts[0].x + pts[2].x) / 2.0,
        (pts[0].y + pts[2].y) / 2.0,
    );
    let center_end = Point::new(
        (pts[1].x + p2_end.x) / 2.0,
        (pts[1].y + p2_end.y) / 2.0,
    );
    let center_stroke =
        create_stroke(ctx.stroke_color.scale_alpha(0.5), 1.0, LineStyle::Dashed);
    frame.stroke(&Path::line(center_start, center_end), center_stroke);

    // Optional fill between channels
    if let Some(fill_color) = drawing.fill_color() {
        let fill = Path::new(|builder| {
            builder.move_to(pts[0]);
            builder.line_to(pts[1]);
            builder.line_to(p2_end);
            builder.line_to(pts[2]);
            builder.close();
        });
        frame.fill(
            &fill,
            fill_color.scale_alpha(drawing.style.fill_opacity * ctx.alpha),
        );
    }
}
