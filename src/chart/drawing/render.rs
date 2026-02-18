//! Drawing Rendering
//!
//! Functions to render drawings and selection handles on the chart canvas.

use super::{Drawing, DrawingManager, DrawingTool, LineStyle};
use crate::chart::ViewState;
use iced::widget::canvas::{Frame, Path, Stroke, LineDash};
use iced::theme::palette::Extended;
use iced::{Color, Point, Size};

/// Handle size in pixels
const HANDLE_SIZE: f32 = 8.0;

/// Selection highlight width addition
const SELECTION_WIDTH_BOOST: f32 = 2.0;

/// Draw all drawings on the chart
pub fn draw_drawings(
    frame: &mut Frame,
    state: &ViewState,
    drawings: &DrawingManager,
    bounds: Size,
    palette: &Extended,
) {
    // Early return if nothing to draw
    if drawings.is_empty() {
        return;
    }

    // Draw completed drawings
    for drawing in drawings.drawings() {
        let is_selected = drawings.is_selected(drawing.id);
        draw_single_drawing(frame, state, drawing, bounds, palette, is_selected);

        if is_selected {
            draw_handles(frame, state, drawing, bounds, palette);
        }
    }

    // Draw pending preview
    if let Some(pending) = drawings.pending() {
        draw_single_drawing(frame, state, pending, bounds, palette, false);
    }
}

/// Draw a single drawing
fn draw_single_drawing(
    frame: &mut Frame,
    state: &ViewState,
    drawing: &Drawing,
    bounds: Size,
    _palette: &Extended,
    is_selected: bool,
) {
    if !drawing.visible || drawing.points.is_empty() {
        return;
    }

    let screen_points: Vec<Point> = drawing
        .points
        .iter()
        .map(|p| p.to_screen(state, bounds))
        .collect();

    let stroke_color = drawing.stroke_color();
    let stroke_width = if is_selected {
        drawing.style.stroke_width + SELECTION_WIDTH_BOOST
    } else {
        drawing.style.stroke_width
    };

    let stroke = create_stroke(stroke_color, stroke_width, drawing.style.line_style);

    match drawing.tool {
        DrawingTool::None => {}
        DrawingTool::HorizontalLine => {
            if let Some(p) = screen_points.first() {
                let path = Path::line(
                    Point::new(0.0, p.y),
                    Point::new(bounds.width, p.y),
                );
                frame.stroke(&path, stroke);
            }
        }
        DrawingTool::VerticalLine => {
            if let Some(p) = screen_points.first() {
                let path = Path::line(
                    Point::new(p.x, 0.0),
                    Point::new(p.x, bounds.height),
                );
                frame.stroke(&path, stroke);
            }
        }
        DrawingTool::Line | DrawingTool::TrendLine => {
            if screen_points.len() >= 2 {
                let path = Path::line(screen_points[0], screen_points[1]);
                frame.stroke(&path, stroke);
            }
        }
        DrawingTool::Ray => {
            if screen_points.len() >= 2 {
                let start = screen_points[0];
                let direction = screen_points[1];

                // Extend to bounds edge
                let end = extend_to_bounds(start, direction, bounds);
                let path = Path::line(start, end);
                frame.stroke(&path, stroke);
            }
        }
        DrawingTool::Rectangle => {
            if screen_points.len() >= 2 {
                let p1 = screen_points[0];
                let p2 = screen_points[1];

                let min_x = p1.x.min(p2.x);
                let min_y = p1.y.min(p2.y);
                let width = (p1.x - p2.x).abs();
                let height = (p1.y - p2.y).abs();

                let rect = iced::Rectangle {
                    x: min_x,
                    y: min_y,
                    width,
                    height,
                };

                // Draw fill if specified
                if let Some(fill_color) = drawing.fill_color() {
                    let fill_path = Path::rectangle(
                        Point::new(rect.x, rect.y),
                        Size::new(rect.width, rect.height),
                    );
                    frame.fill(&fill_path, fill_color.scale_alpha(0.2));
                }

                // Draw stroke
                let stroke_path = Path::rectangle(
                    Point::new(rect.x, rect.y),
                    Size::new(rect.width, rect.height),
                );
                frame.stroke(&stroke_path, stroke);
            }
        }
    }
}

/// Draw selection handles for a drawing
fn draw_handles(
    frame: &mut Frame,
    state: &ViewState,
    drawing: &Drawing,
    bounds: Size,
    palette: &Extended,
) {
    let handles = drawing.handle_positions(state, bounds);
    let handle_half = HANDLE_SIZE / 2.0;

    let fill_color = Color::WHITE;
    let stroke_color = palette.primary.base.color;

    for handle in handles {
        // Draw handle square
        let rect = Path::rectangle(
            Point::new(handle.x - handle_half, handle.y - handle_half),
            Size::new(HANDLE_SIZE, HANDLE_SIZE),
        );

        frame.fill(&rect, fill_color);
        frame.stroke(
            &rect,
            Stroke::default()
                .with_color(stroke_color)
                .with_width(1.5),
        );
    }
}

/// Create a stroke with the given parameters
fn create_stroke(color: Color, width: f32, line_style: LineStyle) -> Stroke<'static> {
    let line_dash = match line_style {
        LineStyle::Solid => LineDash::default(),
        LineStyle::Dashed => LineDash {
            segments: &[8.0, 4.0],
            offset: 0,
        },
        LineStyle::Dotted => LineDash {
            segments: &[2.0, 4.0],
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

/// Extend a ray from start through direction to the bounds edge
fn extend_to_bounds(start: Point, direction: Point, bounds: Size) -> Point {
    let dx = direction.x - start.x;
    let dy = direction.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();

    if len < 0.0001 {
        return direction;
    }

    // Normalize direction
    let dir_x = dx / len;
    let dir_y = dy / len;

    // Find intersection with bounds
    let mut t_max = f32::MAX;

    // Right edge
    if dir_x > 0.0001 {
        let t = (bounds.width - start.x) / dir_x;
        t_max = t_max.min(t);
    }
    // Left edge
    if dir_x < -0.0001 {
        let t = -start.x / dir_x;
        t_max = t_max.min(t);
    }
    // Bottom edge
    if dir_y > 0.0001 {
        let t = (bounds.height - start.y) / dir_y;
        t_max = t_max.min(t);
    }
    // Top edge
    if dir_y < -0.0001 {
        let t = -start.y / dir_y;
        t_max = t_max.min(t);
    }

    Point::new(start.x + dir_x * t_max, start.y + dir_y * t_max)
}
