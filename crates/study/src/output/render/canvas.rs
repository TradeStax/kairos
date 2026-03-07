//! Platform-agnostic 2D drawing surface.
//!
//! The study crate defines this trait; the app crate provides an Iced
//! implementation ([`IcedCanvas`]).

use super::types::{FontHint, LineStyle, TextAlign};
use data::Rgba;

/// Platform-agnostic 2D drawing surface for study renderers.
///
/// All coordinates are in the space provided by the [`ChartView`]
/// implementation — overlay renderers receive chart-space coords,
/// panel renderers receive screen-space coords. The canvas
/// implementation maps these to the actual graphics API.
///
/// [`ChartView`]: super::chart_view::ChartView
pub trait Canvas {
    /// Stroke a single line segment.
    fn stroke_line(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Rgba,
        width: f32,
        style: LineStyle,
    );

    /// Stroke a connected polyline through the given points.
    fn stroke_polyline(
        &mut self,
        points: &[(f32, f32)],
        color: Rgba,
        width: f32,
        style: LineStyle,
    );

    /// Fill a rectangle.
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba);

    /// Stroke a rectangle outline.
    fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba, width: f32);

    /// Fill a closed polygon defined by the given vertices.
    fn fill_polygon(&mut self, points: &[(f32, f32)], color: Rgba);

    /// Fill a circle.
    fn fill_circle(&mut self, cx: f32, cy: f32, radius: f32, color: Rgba);

    /// Stroke a circle outline.
    fn stroke_circle(&mut self, cx: f32, cy: f32, radius: f32, color: Rgba, width: f32);

    /// Draw filled text.
    fn fill_text(
        &mut self,
        x: f32,
        y: f32,
        text: &str,
        size: f32,
        color: Rgba,
        font: FontHint,
    );

    /// Draw filled text with explicit horizontal and vertical alignment.
    fn fill_text_aligned(
        &mut self,
        x: f32,
        y: f32,
        text: &str,
        size: f32,
        color: Rgba,
        font: FontHint,
        align_x: TextAlign,
        align_y: TextAlign,
    );
}
