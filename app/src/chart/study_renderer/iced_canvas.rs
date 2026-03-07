//! Iced canvas adapter implementing the study crate's `Canvas` trait.
//!
//! Bridges the platform-agnostic rendering API to Iced's `Frame`.

use crate::components::primitives::AZERET_MONO;
use iced::widget::canvas::{Frame, LineDash, Path, Stroke, Text};
use iced::{Color, Font, Point, Size};
use study::output::render::{Canvas, FontHint, LineStyle, TextAlign};

/// Iced implementation of the study crate's [`Canvas`] trait.
pub struct IcedCanvas<'a> {
    pub frame: &'a mut Frame,
}

impl<'a> IcedCanvas<'a> {
    pub fn new(frame: &'a mut Frame) -> Self {
        Self { frame }
    }
}

impl Canvas for IcedCanvas<'_> {
    fn stroke_line(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: data::Rgba,
        width: f32,
        style: LineStyle,
    ) {
        let path = Path::line(Point::new(x1, y1), Point::new(x2, y2));
        self.frame.stroke(&path, to_stroke(color, width, style));
    }

    fn stroke_polyline(
        &mut self,
        points: &[(f32, f32)],
        color: data::Rgba,
        width: f32,
        style: LineStyle,
    ) {
        if points.len() < 2 {
            return;
        }
        let path = Path::new(|builder| {
            let (x, y) = points[0];
            builder.move_to(Point::new(x, y));
            for &(x, y) in &points[1..] {
                builder.line_to(Point::new(x, y));
            }
        });
        self.frame.stroke(&path, to_stroke(color, width, style));
    }

    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: data::Rgba) {
        self.frame
            .fill_rectangle(Point::new(x, y), Size::new(w, h), to_color(color));
    }

    fn stroke_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: data::Rgba,
        width: f32,
    ) {
        let path = Path::new(|builder| {
            builder.move_to(Point::new(x, y));
            builder.line_to(Point::new(x + w, y));
            builder.line_to(Point::new(x + w, y + h));
            builder.line_to(Point::new(x, y + h));
            builder.close();
        });
        self.frame.stroke(
            &path,
            Stroke::default()
                .with_color(to_color(color))
                .with_width(width),
        );
    }

    fn fill_polygon(&mut self, points: &[(f32, f32)], color: data::Rgba) {
        if points.len() < 3 {
            return;
        }
        let path = Path::new(|builder| {
            let (x, y) = points[0];
            builder.move_to(Point::new(x, y));
            for &(x, y) in &points[1..] {
                builder.line_to(Point::new(x, y));
            }
            builder.close();
        });
        self.frame.fill(&path, to_color(color));
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, radius: f32, color: data::Rgba) {
        let path = Path::circle(Point::new(cx, cy), radius);
        self.frame.fill(&path, to_color(color));
    }

    fn stroke_circle(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        color: data::Rgba,
        width: f32,
    ) {
        let path = Path::circle(Point::new(cx, cy), radius);
        self.frame.stroke(
            &path,
            Stroke::default()
                .with_color(to_color(color))
                .with_width(width),
        );
    }

    fn fill_text(
        &mut self,
        x: f32,
        y: f32,
        text: &str,
        size: f32,
        color: data::Rgba,
        font: FontHint,
    ) {
        self.frame.fill_text(Text {
            content: text.to_string(),
            position: Point::new(x, y),
            size: iced::Pixels(size),
            color: to_color(color),
            font: match font {
                FontHint::Monospace => AZERET_MONO,
                FontHint::Default => Font::default(),
            },
            ..Text::default()
        });
    }

    fn fill_text_aligned(
        &mut self,
        x: f32,
        y: f32,
        text: &str,
        size: f32,
        color: data::Rgba,
        font: FontHint,
        align_x: TextAlign,
        align_y: TextAlign,
    ) {
        let h_align = match align_x {
            TextAlign::Start => iced::alignment::Horizontal::Left,
            TextAlign::Center => iced::alignment::Horizontal::Center,
            TextAlign::End => iced::alignment::Horizontal::Right,
        };
        let v_align = match align_y {
            TextAlign::Start => iced::alignment::Vertical::Top,
            TextAlign::Center => iced::alignment::Vertical::Center,
            TextAlign::End => iced::alignment::Vertical::Bottom,
        };
        self.frame.fill_text(Text {
            content: text.to_string(),
            position: Point::new(x, y),
            size: iced::Pixels(size),
            color: to_color(color),
            font: match font {
                FontHint::Monospace => AZERET_MONO,
                FontHint::Default => Font::default(),
            },
            align_x: h_align.into(),
            align_y: v_align.into(),
            ..Text::default()
        });
    }
}

/// Convert `Rgba` to Iced `Color`.
#[inline]
fn to_color(c: data::Rgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// Build an Iced `Stroke` from color, width, and line style.
fn to_stroke(color: data::Rgba, width: f32, style: LineStyle) -> Stroke<'static> {
    let dash = match style {
        LineStyle::Solid => LineDash::default(),
        LineStyle::Dashed => LineDash {
            segments: &[6.0, 4.0],
            offset: 0,
        },
        LineStyle::Dotted => LineDash {
            segments: &[2.0, 3.0],
            offset: 0,
        },
    };
    Stroke {
        width,
        line_dash: dash,
        ..Stroke::default()
    }
    .with_color(to_color(color))
}
