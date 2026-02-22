//! Volume Profile drawing renderer.
//!
//! Delegates to the VBP study renderer for the actual profile rendering,
//! falling back to a placeholder rectangle during drag or before computation.

use super::DrawContext;
use super::super::Drawing;
use crate::chart::study_renderer::vbp as vbp_renderer;
use iced::widget::canvas::Frame;
use iced::Point;
use study::Study as _;
use study::output::StudyOutput;

pub fn draw(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    screen_points: &[Point],
) {
    if screen_points.len() < 2 {
        return;
    }

    // If computed VBP data exists, delegate to the study renderer
    if let Some(ref study) = drawing.vbp_study
        && let StudyOutput::Vbp(ref data) = *study.output()
    {
        vbp_renderer::render_vbp(frame, data, ctx.state, ctx.bounds);
        return;
    }

    // Fallback: draw a placeholder rectangle (during drag or before computation)
    super::draw_rect_with_fill(frame, screen_points, drawing, ctx.stroke, ctx.alpha);
}
