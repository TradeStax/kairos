//! Drawing Rendering
//!
//! Functions to render drawings and selection handles on the chart canvas.

use super::{Drawing, DrawingManager, DrawingTool, LineStyle};
use crate::chart::ViewState;
use crate::chart::core::tokens;
use data::{FibonacciConfig, LabelAlignment};
use iced::alignment;
use iced::theme::palette::Extended;
use iced::widget::canvas::{Frame, LineDash, Path, Stroke, Text};
use iced::{Color, Point, Size};

/// Draw only completed drawings (for the drawings cache layer)
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

/// Draw overlay elements (for the crosshair cache layer)
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

/// Draw a single drawing
///
/// `alpha` controls overall opacity (1.0 = fully opaque, 0.5 = preview).
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
        .map(|p| p.to_screen(state, bounds))
        .collect();

    let stroke_color = drawing.stroke_color().scale_alpha(alpha);
    let stroke_width = if is_selected {
        drawing.style.stroke_width + tokens::drawing::SELECTION_WIDTH_BOOST
    } else {
        drawing.style.stroke_width
    };

    let stroke = create_stroke(stroke_color, stroke_width, drawing.style.line_style);

    match drawing.tool {
        DrawingTool::None => {}
        DrawingTool::HorizontalLine => {
            if let Some(p) = screen_points.first() {
                let start = Point::new(0.0, p.y);
                let end = Point::new(bounds.width, p.y);
                let path = Path::line(start, end);
                frame.stroke(&path, stroke);

                draw_drawing_label(frame, drawing, &[start, end], bounds, stroke_color, false);
            }
        }
        DrawingTool::VerticalLine => {
            if let Some(p) = screen_points.first() {
                let path = Path::line(Point::new(p.x, 0.0), Point::new(p.x, bounds.height));
                frame.stroke(&path, stroke);

                draw_drawing_label(
                    frame,
                    drawing,
                    &[Point::new(p.x, 0.0), Point::new(p.x, bounds.height)],
                    bounds,
                    stroke_color,
                    true,
                );
            }
        }
        DrawingTool::Line => {
            if screen_points.len() >= 2 {
                let path = Path::line(screen_points[0], screen_points[1]);
                frame.stroke(&path, stroke);

                draw_drawing_label(
                    frame,
                    drawing,
                    &[screen_points[0], screen_points[1]],
                    bounds,
                    stroke_color,
                    false,
                );
            }
        }
        DrawingTool::Ray => {
            if screen_points.len() >= 2 {
                // 2-point ray: from p0 through p1, extending to bounds
                let forward = extend_to_bounds(screen_points[0], screen_points[1], bounds);
                let path = Path::line(screen_points[0], forward);
                frame.stroke(&path, stroke);

                draw_drawing_label(
                    frame,
                    drawing,
                    &[screen_points[0], forward],
                    bounds,
                    stroke_color,
                    false,
                );
            } else if let Some(p) = screen_points.first() {
                // Legacy 1-point ray: horizontal right
                let end = Point::new(bounds.width, p.y);
                let path = Path::line(*p, end);
                frame.stroke(&path, stroke);

                draw_drawing_label(frame, drawing, &[*p, end], bounds, stroke_color, false);
            }
        }
        DrawingTool::ExtendedLine => {
            if screen_points.len() >= 2 {
                let start = screen_points[0];
                let end = screen_points[1];
                let back = extend_to_bounds(end, start, bounds);
                let forward = extend_to_bounds(start, end, bounds);
                let path = Path::line(back, forward);
                frame.stroke(&path, stroke);

                draw_drawing_label(
                    frame,
                    drawing,
                    &[back, forward],
                    bounds,
                    stroke_color,
                    false,
                );
            }
        }
        DrawingTool::Arrow => {
            if screen_points.len() >= 2 {
                let start = screen_points[0];
                let end = screen_points[1];

                // Draw line
                let path = Path::line(start, end);
                frame.stroke(&path, stroke);

                // Draw arrowhead
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 1.0 {
                    let arrow_len = 12.0_f32;
                    let arrow_width = 5.0_f32;
                    let ux = dx / len;
                    let uy = dy / len;
                    let left = Point::new(
                        end.x - ux * arrow_len + uy * arrow_width,
                        end.y - uy * arrow_len - ux * arrow_width,
                    );
                    let right = Point::new(
                        end.x - ux * arrow_len - uy * arrow_width,
                        end.y - uy * arrow_len + ux * arrow_width,
                    );
                    let arrow = Path::new(|builder| {
                        builder.move_to(end);
                        builder.line_to(left);
                        builder.line_to(right);
                        builder.close();
                    });
                    frame.fill(&arrow, stroke_color);
                }
            }
        }
        DrawingTool::Rectangle => {
            if screen_points.len() >= 2 {
                draw_rect_with_fill(frame, &screen_points, drawing, stroke, alpha);
            }
        }
        DrawingTool::PriceRange => {
            if screen_points.len() >= 2 {
                draw_rect_with_fill(frame, &screen_points, drawing, stroke, alpha);

                // Draw price delta label
                let p1 = &drawing.points[0];
                let p2 = &drawing.points[1];
                let delta = p2.price.units() - p1.price.units();
                let label = format!(
                    "{}{:.2}",
                    if delta >= 0 { "+" } else { "" },
                    delta as f64 / 1e8
                );
                let mid = Point::new(
                    (screen_points[0].x + screen_points[1].x) / 2.0,
                    (screen_points[0].y + screen_points[1].y) / 2.0,
                );
                draw_label(frame, &label, mid, stroke_color);
            }
        }
        DrawingTool::DateRange => {
            if screen_points.len() >= 2 {
                draw_rect_with_fill(frame, &screen_points, drawing, stroke, alpha);

                // Draw time delta label
                let p1 = &drawing.points[0];
                let p2 = &drawing.points[1];
                let t1 = p1.time.min(p2.time);
                let t2 = p1.time.max(p2.time);
                let delta_ms = t2 - t1;
                let label = format_duration(delta_ms);
                let mid = Point::new(
                    (screen_points[0].x + screen_points[1].x) / 2.0,
                    (screen_points[0].y + screen_points[1].y) / 2.0,
                );
                draw_label(frame, &label, mid, stroke_color);
            }
        }
        DrawingTool::Ellipse => {
            if screen_points.len() >= 2 {
                let cx = screen_points[0].x;
                let cy = screen_points[0].y;
                let rx = (screen_points[1].x - cx).abs().max(1.0);
                let ry = (screen_points[1].y - cy).abs().max(1.0);

                let ellipse = Path::new(|builder| {
                    // Approximate ellipse with arcs
                    builder.move_to(Point::new(cx + rx, cy));
                    let steps = 64;
                    for i in 1..=steps {
                        let angle = 2.0 * std::f32::consts::PI * (i as f32 / steps as f32);
                        builder.line_to(Point::new(cx + rx * angle.cos(), cy + ry * angle.sin()));
                    }
                    builder.close();
                });

                // Fill
                if let Some(fill_color) = drawing.fill_color() {
                    frame.fill(
                        &ellipse,
                        fill_color.scale_alpha(drawing.style.fill_opacity * alpha),
                    );
                }

                frame.stroke(&ellipse, stroke);
            }
        }
        DrawingTool::FibRetracement => {
            if screen_points.len() >= 2 {
                let config = drawing
                    .style
                    .fibonacci
                    .as_ref()
                    .cloned()
                    .unwrap_or_default();
                draw_fib_levels(
                    frame,
                    &screen_points[0],
                    &screen_points[1],
                    &config,
                    stroke_width,
                    drawing.style.line_style,
                    bounds,
                );
            }
        }
        DrawingTool::FibExtension => {
            if screen_points.len() >= 3 {
                let config = drawing
                    .style
                    .fibonacci
                    .as_ref()
                    .cloned()
                    .unwrap_or_default();
                // Extension uses points[0..1] for the range, applied from points[2]
                let y_range = screen_points[1].y - screen_points[0].y;
                let min_x = screen_points.iter().map(|p| p.x).fold(f32::MAX, f32::min);
                let max_x = screen_points.iter().map(|p| p.x).fold(f32::MIN, f32::max);

                for level in &config.levels {
                    if !level.visible {
                        continue;
                    }
                    let level_y = screen_points[2].y + y_range * level.ratio as f32;
                    let level_color: Color =
                        crate::style::theme_bridge::rgba_to_iced_color(level.color);
                    let level_stroke =
                        create_stroke(level_color, stroke_width, drawing.style.line_style);

                    let (lx, rx) = if config.extend_lines {
                        (0.0, bounds.width)
                    } else {
                        (min_x, max_x)
                    };

                    let path = Path::line(Point::new(lx, level_y), Point::new(rx, level_y));
                    frame.stroke(&path, level_stroke);

                    if config.show_percentages {
                        draw_label(
                            frame,
                            &level.label,
                            Point::new(rx + 4.0, level_y - 8.0),
                            level_color,
                        );
                    }
                }

                // Draw anchor lines
                let anchor_stroke =
                    create_stroke(stroke_color.scale_alpha(0.4), 1.0, LineStyle::Dashed);
                let path = Path::line(screen_points[0], screen_points[1]);
                frame.stroke(&path, anchor_stroke);
                let path = Path::line(screen_points[1], screen_points[2]);
                frame.stroke(&path, anchor_stroke);
            }
        }
        DrawingTool::ParallelChannel => {
            if screen_points.len() >= 3 {
                // Line 1: points[0] to points[1]
                let path1 = Path::line(screen_points[0], screen_points[1]);
                frame.stroke(&path1, stroke);

                // Line 2: parallel through points[2]
                let dx = screen_points[1].x - screen_points[0].x;
                let dy = screen_points[1].y - screen_points[0].y;
                let p2_start = screen_points[2];
                let p2_end = Point::new(p2_start.x + dx, p2_start.y + dy);
                let path2 = Path::line(p2_start, p2_end);
                frame.stroke(&path2, stroke);

                // Center line (dashed)
                let center_start = Point::new(
                    (screen_points[0].x + screen_points[2].x) / 2.0,
                    (screen_points[0].y + screen_points[2].y) / 2.0,
                );
                let center_end = Point::new(
                    (screen_points[1].x + p2_end.x) / 2.0,
                    (screen_points[1].y + p2_end.y) / 2.0,
                );
                let center_stroke =
                    create_stroke(stroke_color.scale_alpha(0.5), 1.0, LineStyle::Dashed);
                let path3 = Path::line(center_start, center_end);
                frame.stroke(&path3, center_stroke);

                // Optional fill between channels
                if let Some(fill_color) = drawing.fill_color() {
                    let fill = Path::new(|builder| {
                        builder.move_to(screen_points[0]);
                        builder.line_to(screen_points[1]);
                        builder.line_to(p2_end);
                        builder.line_to(p2_start);
                        builder.close();
                    });
                    frame.fill(
                        &fill,
                        fill_color.scale_alpha(drawing.style.fill_opacity * alpha),
                    );
                }
            }
        }
        DrawingTool::TextLabel => {
            if let Some(p) = screen_points.first() {
                let text = drawing
                    .style
                    .text
                    .as_deref()
                    .or(drawing.label.as_deref())
                    .unwrap_or("Text");
                draw_label(frame, text, *p, stroke_color);
            }
        }
        DrawingTool::PriceLabel => {
            if let Some(p) = screen_points.first() {
                let price_units = drawing.points[0].price.units();
                let label = format!("{:.2}", price_units as f64 / 1e8);
                draw_label(frame, &label, *p, stroke_color);
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

/// Create a stroke with the given parameters
fn create_stroke(color: Color, width: f32, line_style: LineStyle) -> Stroke<'static> {
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

/// Extend a ray from start through direction to the bounds edge
fn extend_to_bounds(start: Point, direction: Point, bounds: Size) -> Point {
    let dx = direction.x - start.x;
    let dy = direction.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();

    if len < tokens::epsilon::LINE_DEGENERATE {
        return direction;
    }

    // Normalize direction
    let dir_x = dx / len;
    let dir_y = dy / len;

    // Find intersection with bounds
    let mut t_max = f32::MAX;

    // Right edge
    if dir_x > tokens::epsilon::RAY_DIRECTION {
        let t = (bounds.width - start.x) / dir_x;
        t_max = t_max.min(t);
    }
    // Left edge
    if dir_x < -tokens::epsilon::RAY_DIRECTION {
        let t = -start.x / dir_x;
        t_max = t_max.min(t);
    }
    // Bottom edge
    if dir_y > tokens::epsilon::RAY_DIRECTION {
        let t = (bounds.height - start.y) / dir_y;
        t_max = t_max.min(t);
    }
    // Top edge
    if dir_y < -tokens::epsilon::RAY_DIRECTION {
        let t = -start.y / dir_y;
        t_max = t_max.min(t);
    }

    // Clamp t_max to prevent NaN/infinity when no intersection is found
    let max_extent = bounds.width + bounds.height;
    let t_max = if t_max == f32::MAX || t_max.is_nan() || t_max.is_infinite() {
        max_extent
    } else {
        t_max.min(max_extent)
    };

    Point::new(start.x + dir_x * t_max, start.y + dir_y * t_max)
}

/// Draw a rectangle with optional fill (shared by Rectangle, PriceRange, DateRange)
fn draw_rect_with_fill(
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

    // Draw fill if specified
    if let Some(fill_color) = drawing.fill_color() {
        let fill_path = Path::rectangle(origin, size);
        frame.fill(
            &fill_path,
            fill_color.scale_alpha(drawing.style.fill_opacity * alpha),
        );
    }

    // Draw stroke
    let stroke_path = Path::rectangle(origin, size);
    frame.stroke(&stroke_path, stroke);
}

/// Draw fibonacci retracement levels between two points
fn draw_fib_levels(
    frame: &mut Frame,
    p1: &Point,
    p2: &Point,
    config: &FibonacciConfig,
    stroke_width: f32,
    line_style: LineStyle,
    bounds: Size,
) {
    let y_range = p2.y - p1.y;
    let min_x = p1.x.min(p2.x);
    let max_x = p1.x.max(p2.x);

    for level in &config.levels {
        if !level.visible {
            continue;
        }

        let level_y = p1.y + y_range * level.ratio as f32;
        let level_color: Color =
            crate::style::theme_bridge::rgba_to_iced_color(level.color);
        let level_stroke = create_stroke(level_color, stroke_width, line_style);

        let (lx, rx) = if config.extend_lines {
            (0.0, bounds.width)
        } else {
            (min_x, max_x)
        };

        let path = Path::line(Point::new(lx, level_y), Point::new(rx, level_y));
        frame.stroke(&path, level_stroke);

        // Draw level label
        if config.show_percentages {
            draw_label(
                frame,
                &level.label,
                Point::new(rx + 4.0, level_y - 8.0),
                level_color,
            );
        }
    }

    // Draw fill between levels
    let fill_color = Color::from_rgba(0.5, 0.5, 0.5, 0.05);
    let fill_path = Path::rectangle(
        Point::new(min_x, p1.y.min(p2.y)),
        Size::new(max_x - min_x, (p2.y - p1.y).abs()),
    );
    frame.fill(&fill_path, fill_color);
}

/// Draw a text label at a given position
fn draw_label(frame: &mut Frame, text: &str, position: Point, color: Color) {
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

/// Y offset to position label above the line
const LABEL_Y_OFFSET: f32 = 4.0;
/// X padding from chart edges for labels
const LABEL_X_PADDING: f32 = 6.0;
/// Y padding from top of chart for vertical line labels
const LABEL_TOP_PADDING: f32 = 8.0;

/// Draw user-provided label on a line-type drawing
///
/// `line_endpoints` are the visible start/end of the line in screen coords.
/// `is_vertical` indicates vertical line (label positioning adapts).
fn draw_drawing_label(
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
        // Vertical line: label near the top, positioned relative to line
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
        // Horizontal / angled line
        let p0 = line_endpoints[0];
        let p1 = line_endpoints[1];

        // Sort by x to get left/right endpoints
        let (left, right) = if p0.x <= p1.x { (p0, p1) } else { (p1, p0) };
        let mid = Point::new((left.x + right.x) / 2.0, (left.y + right.y) / 2.0);

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

/// Format a duration in milliseconds to a human-readable string
fn format_duration(ms: u64) -> String {
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
