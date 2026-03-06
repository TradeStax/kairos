//! Platform-agnostic drawing API for custom study rendering.
//!
//! [`DrawContext`] provides a minimal set of drawing primitives that
//! studies can use without depending on any GUI framework. The app
//! crate provides a concrete implementation wrapping the Iced canvas.

use data::SerializableColor;

/// Platform-agnostic drawing context for custom study rendering.
///
/// Studies that implement [`CustomDrawStudy`](super::capabilities::CustomDrawStudy)
/// receive a `&mut dyn DrawContext` and can draw arbitrary visuals
/// using these primitives. The chart engine maps study coordinates
/// (interval × price) to screen coordinates automatically.
pub trait DrawContext {
    /// Draw a line between two points in study coordinates.
    fn draw_line(
        &mut self,
        x1: u64,
        y1: f64,
        x2: u64,
        y2: f64,
        color: SerializableColor,
        width: f32,
    );

    /// Fill a rectangle in study coordinates.
    fn fill_rect(
        &mut self,
        x: u64,
        y_top: f64,
        width_intervals: u32,
        y_bottom: f64,
        color: SerializableColor,
    );

    /// Draw text at a study coordinate position.
    fn draw_text(&mut self, x: u64, y: f64, text: &str, color: SerializableColor, size: f32);

    /// Draw a circle at a study coordinate position.
    fn draw_circle(
        &mut self,
        x: u64,
        y: f64,
        radius: f32,
        fill: SerializableColor,
        stroke: Option<(SerializableColor, f32)>,
    );

    /// Current visible interval range (start, end).
    fn visible_range(&self) -> (u64, u64);

    /// Current visible price range (min, max).
    fn visible_price_range(&self) -> (f64, f64);

    /// Width of a single candle cell in pixels.
    fn cell_width(&self) -> f32;

    /// Height of a single tick step in pixels.
    fn cell_height(&self) -> f32;

    /// Current level-of-detail (0 = full detail, higher = less detail).
    fn lod_level(&self) -> u8;
}
