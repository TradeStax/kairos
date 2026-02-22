//! Drawing Rendering
//!
//! Functions to render drawings and selection handles on the chart canvas.
//! Tool-specific rendering is delegated to submodules.

mod annotations;
mod calculator;
mod channel;
mod fibonacci;
mod lines;
mod shapes;
mod volume_profile;

use super::{Drawing, DrawingManager, DrawingTool};
use crate::chart::ViewState;
use crate::chart::core::tokens;
use data::{LabelAlignment, LineStyle};
use iced::alignment;
use iced::theme::palette::Extended;
use iced::widget::canvas::{Frame, LineDash, Path, Stroke, Text};
use iced::{Color, Point, Size};

/// Shared rendering context passed to per-tool draw functions.
pub struct DrawContext<'a> {
    pub state: &'a ViewState,
    pub bounds: Size,
    pub stroke_color: Color,
    pub stroke_width: f32,
    pub stroke: Stroke<'a>,
    pub is_selected: bool,
    pub alpha: f32,
}

/// Draw only completed drawings (for the drawings cache layer).
///
/// Renders all finalized drawings. This is cached separately from the
/// crosshair layer so completed drawings aren't re-rendered every cursor move.
pub fn draw_completed_drawings(
    frame: &mut Frame,
    state: &ViewState,
    drawings: &DrawingManager,
    bounds: Size,
    palette: &Extended,
) {
    for drawing in drawings.drawings() {
        let is_selected = drawings.is_selected(drawing.id);
        draw_single_drawing(frame, state, drawing, bounds, palette, is_selected, 1.0);
    }
}

/// Draw overlay elements (for the crosshair cache layer).
///
/// Renders selection handles on selected drawings and the pending
/// preview drawing with reduced alpha.
pub fn draw_overlay_drawings(
    frame: &mut Frame,
    state: &ViewState,
    drawings: &DrawingManager,
    bounds: Size,
    palette: &Extended,
) {
    // Draw selection handles for selected drawings
    for drawing in drawings.drawings() {
        if drawings.is_selected(drawing.id) {
            draw_handles(frame, state, drawing, bounds, palette);
        }
    }

    // Draw pending preview with reduced alpha
    if let Some(pending) = drawings.pending() {
        draw_single_drawing(
            frame,
            state,
            pending,
            bounds,
            palette,
            false,
            tokens::drawing::PREVIEW_ALPHA,
        );
    }

    // Draw clone placement preview with reduced alpha
    if let Some(clone) = drawings.clone_preview() {
        draw_single_drawing(
            frame,
            state,
            clone,
            bounds,
            palette,
            false,
            tokens::drawing::PREVIEW_ALPHA,
        );
    }
}

/// Draw a single drawing, dispatching to the appropriate submodule.
///
/// `alpha` controls overall opacity (1.0 = fully opaque, <1.0 = preview).
fn draw_single_drawing(
    frame: &mut Frame,
    state: &ViewState,
    drawing: &Drawing,
    bounds: Size,
    _palette: &Extended,
    is_selected: bool,
    alpha: f32,
) {
    if !drawing.visible || drawing.points.is_empty() {
        return;
    }

    let screen_points: Vec<Point> = drawing
        .points
        .iter()
        .map(|p| p.as_screen_point(state, bounds))
        .collect();

    let stroke_color = drawing.stroke_color().scale_alpha(alpha);
    let stroke_width = if is_selected {
        drawing.style.stroke_width + tokens::drawing::SELECTION_WIDTH_BOOST
    } else {
        drawing.style.stroke_width
    };

    // Draw selection glow (soft halo) behind selected drawings
    if is_selected {
        let glow_width = stroke_width + tokens::selection::GLOW_EXTRA;
        let glow_color = stroke_color.scale_alpha(tokens::selection::GLOW_ALPHA);
        let glow_stroke = create_stroke(glow_color, glow_width, drawing.style.line_style);

        let glow_ctx = DrawContext {
            state,
            bounds,
            stroke_color: glow_color,
            stroke_width: glow_width,
            stroke: glow_stroke,
            is_selected,
            alpha,
        };

        // Render glow pass first (behind the main drawing)
        dispatch_draw(frame, &glow_ctx, drawing, &screen_points);
    }

    let stroke = create_stroke(stroke_color, stroke_width, drawing.style.line_style);

    let ctx = DrawContext {
        state,
        bounds,
        stroke_color,
        stroke_width,
        stroke,
        is_selected,
        alpha,
    };

    dispatch_draw(frame, &ctx, drawing, &screen_points);
}

/// Dispatch drawing to the appropriate tool-specific renderer.
fn dispatch_draw(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    screen_points: &[Point],
) {
    match drawing.tool {
        DrawingTool::None => {}

        DrawingTool::HorizontalLine
        | DrawingTool::VerticalLine
        | DrawingTool::Line
        | DrawingTool::Ray
        | DrawingTool::ExtendedLine
        | DrawingTool::Arrow => lines::draw(frame, ctx, drawing, screen_points),

        DrawingTool::Rectangle
        | DrawingTool::PriceRange
        | DrawingTool::DateRange
        | DrawingTool::Ellipse => shapes::draw(frame, ctx, drawing, screen_points),

        DrawingTool::FibRetracement | DrawingTool::FibExtension => {
            fibonacci::draw(frame, ctx, drawing, screen_points)
        }

        DrawingTool::ParallelChannel => {
            channel::draw(frame, ctx, drawing, screen_points)
        }

        DrawingTool::TextLabel | DrawingTool::PriceLabel => {
            annotations::draw(frame, ctx, drawing, screen_points)
        }

        DrawingTool::BuyCalculator | DrawingTool::SellCalculator => {
            calculator::draw(frame, ctx, drawing, screen_points)
        }

        DrawingTool::VolumeProfile => {
            volume_profile::draw(frame, ctx, drawing, screen_points)
        }
    }
}

/// Draw selection handles for a drawing.
fn draw_handles(
    frame: &mut Frame,
    state: &ViewState,
    drawing: &Drawing,
    bounds: Size,
    _palette: &Extended,
) {
    let handles = drawing.handle_positions(state, bounds);
    let handle_half = tokens::drawing::HANDLE_SIZE / 2.0;
    let stroke_color = Color::from_rgb(0.7, 0.7, 0.7);

    for handle in handles {
        let circle = Path::circle(handle, handle_half);
        frame.stroke(
            &circle,
            Stroke::default().with_color(stroke_color).with_width(1.5),
        );
    }
}

// ---------------------------------------------------------------------------
// Shared helpers used by submodules
// ---------------------------------------------------------------------------

/// Create a stroke with the given color, width, and line style.
pub(super) fn create_stroke(
    color: Color,
    width: f32,
    line_style: LineStyle,
) -> Stroke<'static> {
    let line_dash = match line_style {
        LineStyle::Solid => LineDash::default(),
        LineStyle::Dashed => LineDash {
            segments: tokens::overlay::DASH_PATTERN,
            offset: 0,
        },
        LineStyle::Dotted => LineDash {
            segments: tokens::overlay::DOT_PATTERN,
            offset: 0,
        },
        LineStyle::DashDot => LineDash {
            segments: tokens::overlay::DASH_DOT_PATTERN,
            offset: 0,
        },
    };

    Stroke::with_color(
        Stroke {
            width,
            line_dash,
            ..Default::default()
        },
        color,
    )
}

/// Extend a ray from `start` through `direction` to the bounds edge.
pub(super) fn extend_to_bounds(start: Point, direction: Point, bounds: Size) -> Point {
    let dx = direction.x - start.x;
    let dy = direction.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();

    if len < tokens::epsilon::LINE_DEGENERATE {
        return direction;
    }

    let dir_x = dx / len;
    let dir_y = dy / len;

    let mut t_max = f32::MAX;

    if dir_x > tokens::epsilon::RAY_DIRECTION {
        let t = (bounds.width - start.x) / dir_x;
        t_max = t_max.min(t);
    }
    if dir_x < -tokens::epsilon::RAY_DIRECTION {
        let t = -start.x / dir_x;
        t_max = t_max.min(t);
    }
    if dir_y > tokens::epsilon::RAY_DIRECTION {
        let t = (bounds.height - start.y) / dir_y;
        t_max = t_max.min(t);
    }
    if dir_y < -tokens::epsilon::RAY_DIRECTION {
        let t = -start.y / dir_y;
        t_max = t_max.min(t);
    }

    let max_extent = bounds.width + bounds.height;
    let t_max = if t_max == f32::MAX || t_max.is_nan() || t_max.is_infinite() {
        max_extent
    } else {
        t_max.min(max_extent)
    };

    Point::new(start.x + dir_x * t_max, start.y + dir_y * t_max)
}

/// Draw a rectangle with optional fill (shared by Rectangle, PriceRange, DateRange).
pub(super) fn draw_rect_with_fill(
    frame: &mut Frame,
    screen_points: &[Point],
    drawing: &Drawing,
    stroke: Stroke<'_>,
    alpha: f32,
) {
    let p1 = screen_points[0];
    let p2 = screen_points[1];

    let min_x = p1.x.min(p2.x);
    let min_y = p1.y.min(p2.y);
    let width = (p1.x - p2.x).abs();
    let height = (p1.y - p2.y).abs();

    let origin = Point::new(min_x, min_y);
    let size = Size::new(width, height);

    if let Some(fill_color) = drawing.fill_color() {
        let fill_path = Path::rectangle(origin, size);
        frame.fill(
            &fill_path,
            fill_color.scale_alpha(drawing.style.fill_opacity * alpha),
        );
    }

    let stroke_path = Path::rectangle(origin, size);
    frame.stroke(&stroke_path, stroke);
}

/// Draw a text label at a given position (11px, default alignment).
pub(super) fn draw_label(frame: &mut Frame, text: &str, position: Point, color: Color) {
    let label = Text {
        content: text.to_string(),
        position,
        color,
        size: iced::Pixels(11.0),
        ..Default::default()
    };
    frame.fill_text(label);
}

/// Draw a text label with horizontal alignment control.
/// Text is bottom-aligned so the baseline sits at `position.y`.
fn draw_label_aligned(
    frame: &mut Frame,
    text: &str,
    position: Point,
    color: Color,
    h_align: alignment::Horizontal,
) {
    let label = Text {
        content: text.to_string(),
        position,
        color,
        size: iced::Pixels(11.0),
        align_x: h_align.into(),
        align_y: alignment::Vertical::Bottom.into(),
        ..Default::default()
    };
    frame.fill_text(label);
}

/// Draw a text label at a given position with custom font size and alpha.
pub(super) fn draw_calc_label(
    frame: &mut Frame,
    text: &str,
    position: Point,
    color: Color,
    font_size: f32,
    alpha: f32,
) {
    let label = Text {
        content: text.to_string(),
        position,
        color: color.scale_alpha(alpha),
        size: iced::Pixels(font_size),
        ..Default::default()
    };
    frame.fill_text(label);
}

/// Y offset to position label above the line.
const LABEL_Y_OFFSET: f32 = tokens::label::Y_OFFSET;
/// X padding from chart edges for labels.
const LABEL_X_PADDING: f32 = tokens::label::X_PADDING;
/// Y padding from top of chart for vertical line labels.
const LABEL_TOP_PADDING: f32 = 8.0;

/// Draw user-provided label on a line-type drawing.
///
/// `line_endpoints` are the visible start/end of the line in screen coords.
/// `is_vertical` indicates vertical line (label positioning adapts).
pub(super) fn draw_drawing_label(
    frame: &mut Frame,
    drawing: &Drawing,
    line_endpoints: &[Point],
    bounds: Size,
    stroke_color: Color,
    is_vertical: bool,
) {
    if !drawing.style.show_labels {
        return;
    }
    let label_text = match drawing.label.as_deref() {
        Some(text) if !text.is_empty() => text,
        _ => return,
    };

    if line_endpoints.len() < 2 {
        return;
    }

    let align = drawing.style.label_alignment;

    if is_vertical {
        let x = line_endpoints[0].x;
        let (pos, h_align) = match align {
            LabelAlignment::Left => (
                Point::new(x - LABEL_X_PADDING, LABEL_TOP_PADDING),
                alignment::Horizontal::Right,
            ),
            LabelAlignment::Center => (
                Point::new(x, LABEL_TOP_PADDING),
                alignment::Horizontal::Center,
            ),
            LabelAlignment::Right => (
                Point::new(x + LABEL_X_PADDING, LABEL_TOP_PADDING),
                alignment::Horizontal::Left,
            ),
        };
        draw_label_aligned(frame, label_text, pos, stroke_color, h_align);
    } else {
        let p0 = line_endpoints[0];
        let p1 = line_endpoints[1];

        let (left, right) = if p0.x <= p1.x { (p0, p1) } else { (p1, p0) };
        let mid = Point::new(
            (left.x + right.x) / 2.0,
            (left.y + right.y) / 2.0,
        );

        let (pos, h_align) = match align {
            LabelAlignment::Left => (
                Point::new(left.x.max(LABEL_X_PADDING), left.y - LABEL_Y_OFFSET),
                alignment::Horizontal::Left,
            ),
            LabelAlignment::Center => (
                Point::new(mid.x, mid.y - LABEL_Y_OFFSET),
                alignment::Horizontal::Center,
            ),
            LabelAlignment::Right => (
                Point::new(
                    right.x.min(bounds.width - LABEL_X_PADDING),
                    right.y - LABEL_Y_OFFSET,
                ),
                alignment::Horizontal::Right,
            ),
        };
        draw_label_aligned(frame, label_text, pos, stroke_color, h_align);
    }
}

/// Format a duration in milliseconds to a human-readable string.
pub(super) fn format_duration(ms: u64) -> String {
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        format!("{}d {}h", days, hours % 24)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes % 60)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds % 60)
    } else {
        format!("{}s", seconds)
    }
}
