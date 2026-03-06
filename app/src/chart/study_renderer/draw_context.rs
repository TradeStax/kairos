//! Concrete [`DrawContext`] implementation wrapping Iced's canvas `Frame`.
//!
//! Maps study-space coordinates (interval, price) to screen-space pixels
//! using the chart's [`ViewState`] coordinate helpers.

use crate::chart::ViewState;
use data::SerializableColor;
use iced::widget::canvas::{self, Frame, Path, Stroke, Text};
use iced::{Point, Size};
use study::StudyPlacement;
use study::core::draw_context::DrawContext;

/// Iced-backed implementation of the study [`DrawContext`] trait.
///
/// Wraps a mutable `Frame` reference and maps study coordinates to
/// screen pixels using the chart's `ViewState`.
#[allow(dead_code)] // Ready for use by CustomDrawStudy implementors
pub struct IcedDrawContext<'a> {
    frame: &'a mut Frame,
    state: &'a ViewState,
    bounds: Size,
    placement: StudyPlacement,
}

#[allow(dead_code)] // Ready for use by CustomDrawStudy implementors
impl<'a> IcedDrawContext<'a> {
    pub fn new(
        frame: &'a mut Frame,
        state: &'a ViewState,
        bounds: Size,
        placement: StudyPlacement,
    ) -> Self {
        Self {
            frame,
            state,
            bounds,
            placement,
        }
    }

    /// Convert a study interval to screen X pixel.
    fn to_screen_x(&self, interval: u64) -> f32 {
        let chart_x = self.state.interval_to_x(interval);
        (chart_x + self.state.translation.x) * self.state.scaling + self.bounds.width / 2.0
    }

    /// Convert a study price/value to screen Y pixel.
    fn to_screen_y(&self, value: f64) -> f32 {
        match self.placement {
            StudyPlacement::Overlay
            | StudyPlacement::Background
            | StudyPlacement::CandleReplace
            | StudyPlacement::SidePanel => {
                // Use the chart's price-to-Y mapping
                let price = data::Price::from_f64(value);
                let chart_y = self.state.price_to_y(price);
                (chart_y + self.state.translation.y) * self.state.scaling + self.bounds.height / 2.0
            }
            StudyPlacement::Panel => {
                // Panel studies use a local 0..height mapping;
                // the caller is responsible for normalizing to [0,1]
                value as f32 * self.bounds.height
            }
        }
    }

    fn to_iced_color(color: SerializableColor) -> iced::Color {
        iced::Color {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

impl DrawContext for IcedDrawContext<'_> {
    fn draw_line(
        &mut self,
        x1: u64,
        y1: f64,
        x2: u64,
        y2: f64,
        color: SerializableColor,
        width: f32,
    ) {
        let p1 = Point::new(self.to_screen_x(x1), self.to_screen_y(y1));
        let p2 = Point::new(self.to_screen_x(x2), self.to_screen_y(y2));
        let stroke = Stroke::default()
            .with_color(Self::to_iced_color(color))
            .with_width(width);
        self.frame.stroke(&Path::line(p1, p2), stroke);
    }

    fn fill_rect(
        &mut self,
        x: u64,
        y_top: f64,
        width_intervals: u32,
        y_bottom: f64,
        color: SerializableColor,
    ) {
        let sx = self.to_screen_x(x);
        let sy_top = self.to_screen_y(y_top);
        let sy_bottom = self.to_screen_y(y_bottom);

        // Width in pixels: approximate by using cell_width * scaling * count
        let w = self.state.cell_width * self.state.scaling * width_intervals as f32;
        let h = (sy_bottom - sy_top).abs();
        let top = sy_top.min(sy_bottom);

        self.frame.fill_rectangle(
            Point::new(sx, top),
            Size::new(w, h),
            Self::to_iced_color(color),
        );
    }

    fn draw_text(&mut self, x: u64, y: f64, text: &str, color: SerializableColor, size: f32) {
        self.frame.fill_text(Text {
            content: text.to_string(),
            position: Point::new(self.to_screen_x(x), self.to_screen_y(y)),
            size: iced::Pixels(size),
            color: Self::to_iced_color(color),
            ..canvas::Text::default()
        });
    }

    fn draw_circle(
        &mut self,
        x: u64,
        y: f64,
        radius: f32,
        fill: SerializableColor,
        stroke: Option<(SerializableColor, f32)>,
    ) {
        let center = Point::new(self.to_screen_x(x), self.to_screen_y(y));
        let circle = Path::circle(center, radius);
        self.frame.fill(&circle, Self::to_iced_color(fill));
        if let Some((stroke_color, stroke_width)) = stroke {
            let s = Stroke::default()
                .with_color(Self::to_iced_color(stroke_color))
                .with_width(stroke_width);
            self.frame.stroke(&circle, s);
        }
    }

    fn visible_range(&self) -> (u64, u64) {
        let region = self.state.visible_region(self.bounds);
        self.state.interval_range(&region)
    }

    fn visible_price_range(&self) -> (f64, f64) {
        let region = self.state.visible_region(self.bounds);
        let (high, low) = self.state.price_range(&region);
        (low.to_f64(), high.to_f64())
    }

    fn cell_width(&self) -> f32 {
        self.state.cell_width
    }

    fn cell_height(&self) -> f32 {
        self.state.cell_height
    }

    fn lod_level(&self) -> u8 {
        0 // Full detail — LOD system not yet wired
    }
}
