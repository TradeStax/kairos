//! Volume Profile drawing renderer.
//!
//! Delegates to the VBP study renderer for the actual profile rendering,
//! falling back to a placeholder rectangle during drag or before computation.

use super::super::Drawing;
use crate::chart::ViewState;
use crate::chart::study_renderer::vbp as vbp_renderer;
use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Color, Point, Size};
use study::Study as _;
use study::output::StudyOutput;

/// Render a VBP drawing in chart coordinates (frame has transforms applied).
///
/// The main chart frame applies `translate(center) + scale + translate(translation)`
/// before calling this, so `interval_to_x()` and `price_to_y()` produce
/// correct coordinates for the VBP renderer.
pub fn draw_in_chart_coords(
    frame: &mut Frame,
    state: &ViewState,
    drawing: &Drawing,
    bounds: Size,
    is_selected: bool,
) {
    if !drawing.visible || drawing.points.len() < 2 {
        return;
    }

    // If computed profile data exists, delegate to the renderer
    if let Some(ref study) = drawing.vbp_study
        && let StudyOutput::Profile(ref profiles, ref config) =
            *study.output()
    {
        for output in profiles {
            let (ax, br) = vbp_renderer::profile_x_range(
                output, state, bounds,
            );
            vbp_renderer::render_vbp(
                frame, output, config, state, bounds, ax, br,
            );
        }

        // Draw selection highlight
        if is_selected {
            draw_selection_rect(frame, state, drawing);
        }
        return;
    }

    // Fallback: draw a placeholder rectangle in chart coordinates
    let x1 = state.interval_to_x(drawing.points[0].time);
    let x2 = state.interval_to_x(drawing.points[1].time);
    let y1 = state.price_to_y(drawing.points[0].price);
    let y2 = state.price_to_y(drawing.points[1].price);

    let min_x = x1.min(x2);
    let min_y = y1.min(y2);
    let width = (x1 - x2).abs();
    let height = (y1 - y2).abs();

    if width > 0.0 && height > 0.0 {
        let stroke_color = drawing.stroke_color().scale_alpha(0.5);
        let rect = Path::rectangle(
            Point::new(min_x, min_y),
            Size::new(width, height),
        );
        if let Some(fill_color) = drawing.fill_color() {
            frame.fill(
                &rect,
                fill_color.scale_alpha(drawing.style.fill_opacity),
            );
        }
        frame.stroke(
            &rect,
            Stroke::default()
                .with_color(stroke_color)
                .with_width(1.0),
        );
    }
}

/// Draw a subtle selection highlight around a VBP drawing.
fn draw_selection_rect(
    frame: &mut Frame,
    state: &ViewState,
    drawing: &Drawing,
) {
    if drawing.points.len() < 2 {
        return;
    }
    let x1 = state.interval_to_x(drawing.points[0].time);
    let x2 = state.interval_to_x(drawing.points[1].time);
    let y1 = state.price_to_y(drawing.points[0].price);
    let y2 = state.price_to_y(drawing.points[1].price);

    let min_x = x1.min(x2);
    let min_y = y1.min(y2);
    let width = (x1 - x2).abs();
    let height = (y1 - y2).abs();

    if width > 0.0 && height > 0.0 {
        let rect = Path::rectangle(
            Point::new(min_x, min_y),
            Size::new(width, height),
        );
        frame.stroke(
            &rect,
            Stroke::default()
                .with_color(Color {
                    r: 0.7,
                    g: 0.7,
                    b: 0.7,
                    a: 0.6,
                })
                .with_width(1.5),
        );
    }
}
