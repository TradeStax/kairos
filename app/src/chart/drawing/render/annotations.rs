//! Annotation drawing rendering: TextLabel, PriceLabel.

use super::{DrawContext, draw_label};
use super::super::Drawing;
use data::DrawingTool;
use iced::Point;
use iced::widget::canvas::Frame;

pub fn draw(frame: &mut Frame, ctx: &DrawContext<'_>, drawing: &Drawing, pts: &[Point]) {
    match drawing.tool {
        DrawingTool::TextLabel => draw_text_label(frame, ctx, drawing, pts),
        DrawingTool::PriceLabel => draw_price_label(frame, ctx, drawing, pts),
        _ => {}
    }
}

fn draw_text_label(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    if let Some(p) = pts.first() {
        let text = drawing
            .style
            .text
            .as_deref()
            .or(drawing.label.as_deref())
            .unwrap_or("Text");
        draw_label(frame, text, *p, ctx.stroke_color);
    }
}

fn draw_price_label(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    if let Some(p) = pts.first() {
        let label = format!("{:.2}", drawing.points[0].price.to_f64());
        draw_label(frame, &label, *p, ctx.stroke_color);
    }
}
