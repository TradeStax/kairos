//! Annotation drawing rendering: TextLabel, PriceLabel.

use super::super::Drawing;
use super::DrawContext;
use data::DrawingTool;
use iced::alignment;
use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::{Point, Size};

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
    let Some(p) = pts.first() else { return };

    let text_content = drawing
        .style
        .text
        .as_deref()
        .or(drawing.label.as_deref())
        .unwrap_or("Text");

    let font_size = drawing.style.text_font_size;
    let char_count = text_content.chars().count() as f32;
    let text_w = (char_count * font_size * 0.55).max(20.0);
    let text_h = font_size * 1.3;
    let pad = 4.0;

    let box_origin = Point::new(p.x, p.y);
    let box_size = Size::new(text_w + pad * 2.0, text_h + pad);

    // Anchor dot
    let dot = Path::circle(*p, 2.5);
    frame.fill(&dot, ctx.stroke_color.scale_alpha(ctx.alpha));

    // Background box (if fill is set)
    if let Some(fill_color) = drawing.fill_color() {
        let bg_rect = Path::rectangle(box_origin, box_size);
        frame.fill(
            &bg_rect,
            fill_color.scale_alpha(drawing.style.fill_opacity * ctx.alpha),
        );
        let border_rect = Path::rectangle(box_origin, box_size);
        frame.stroke(
            &border_rect,
            Stroke::default()
                .with_color(ctx.stroke_color.scale_alpha(ctx.alpha))
                .with_width(1.0),
        );
    }

    // Text
    frame.fill_text(Text {
        content: text_content.to_string(),
        position: Point::new(p.x + pad, p.y + pad * 0.5),
        color: ctx.stroke_color.scale_alpha(ctx.alpha),
        size: iced::Pixels(font_size),
        ..Default::default()
    });
}

fn draw_price_label(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    let Some(p) = pts.first() else { return };

    let price_str = format!("{:.2}", drawing.points[0].price.to_f64());
    let label = match drawing.label.as_deref() {
        Some(lbl) if !lbl.is_empty() => lbl.to_string(),
        _ => price_str,
    };

    let font_size = 12.0f32;
    let char_count = label.chars().count() as f32;
    let badge_w = (char_count * font_size * 0.55).max(40.0) + 12.0;
    let badge_h = font_size * 1.4;
    let gap = 4.0;

    // Vertical tick mark (pin downward from anchor)
    let tick_len = 6.0;
    frame.stroke(
        &Path::line(*p, Point::new(p.x, p.y + tick_len)),
        Stroke::default()
            .with_color(ctx.stroke_color.scale_alpha(ctx.alpha))
            .with_width(ctx.stroke_width),
    );

    // Badge
    let badge_origin = Point::new(p.x + gap, p.y);
    let badge_size = Size::new(badge_w, badge_h);

    // Background fill
    let bg_path = Path::rectangle(badge_origin, badge_size);
    if let Some(fill_color) = drawing.fill_color() {
        frame.fill(
            &bg_path,
            fill_color.scale_alpha(drawing.style.fill_opacity * ctx.alpha),
        );
    } else {
        frame.fill(&bg_path, ctx.stroke_color.scale_alpha(0.15 * ctx.alpha));
    }

    // Badge border
    let border_path = Path::rectangle(badge_origin, badge_size);
    frame.stroke(
        &border_path,
        Stroke::default()
            .with_color(ctx.stroke_color.scale_alpha(ctx.alpha))
            .with_width(1.0),
    );

    // Text (vertically centered inside badge)
    frame.fill_text(Text {
        content: label,
        position: Point::new(badge_origin.x + 6.0, badge_origin.y + badge_h * 0.5),
        color: ctx.stroke_color.scale_alpha(ctx.alpha),
        size: iced::Pixels(font_size),
        align_y: alignment::Vertical::Center.into(),
        ..Default::default()
    });
}
