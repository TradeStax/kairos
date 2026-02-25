//! Per-tool hit testing
//!
//! Each function performs geometric distance testing for a specific drawing
//! tool category. The main entry point is [`hit_test_tool`], which dispatches
//! to the appropriate function based on `DrawingTool`.

use data::{DrawingStyle, DrawingTool};
use iced::{Point, Size};

/// Test whether `cursor` is within `tolerance` of a drawing defined by
/// `screen_points` for the given tool.
pub fn hit_test_tool(
    tool: DrawingTool,
    screen_points: &[Point],
    cursor: Point,
    tolerance: f32,
    style: &DrawingStyle,
    bounds: Size,
) -> bool {
    match tool {
        DrawingTool::None => false,
        DrawingTool::HorizontalLine => hit_test_horizontal(screen_points, cursor, tolerance),
        DrawingTool::VerticalLine => hit_test_vertical(screen_points, cursor, tolerance),
        DrawingTool::Line | DrawingTool::Arrow => {
            hit_test_line_segment(screen_points, cursor, tolerance)
        }
        DrawingTool::Ray => hit_test_ray(screen_points, cursor, tolerance),
        DrawingTool::ExtendedLine => {
            hit_test_extended_line(screen_points, cursor, tolerance)
        }
        DrawingTool::Rectangle => hit_test_rect(screen_points, cursor, tolerance, style),
        DrawingTool::Ellipse => hit_test_ellipse(screen_points, cursor, tolerance, style),
        DrawingTool::FibRetracement => {
            hit_test_fib_retracement(screen_points, cursor, tolerance, style, bounds)
        }
        DrawingTool::FibExtension => {
            hit_test_fib_extension(screen_points, cursor, tolerance, style)
        }
        DrawingTool::ParallelChannel => {
            hit_test_channel(screen_points, cursor, tolerance)
        }
        DrawingTool::TextLabel | DrawingTool::PriceLabel => {
            hit_test_text(screen_points, cursor, style)
        }
        DrawingTool::BuyCalculator | DrawingTool::SellCalculator => {
            hit_test_calculator(screen_points, cursor, tolerance)
        }
        DrawingTool::VolumeProfile => {
            hit_test_rect_outline_only(screen_points, cursor, tolerance)
        }
        DrawingTool::AiContext => hit_test_rect(screen_points, cursor, tolerance, style),
    }
}

// ── Lines ────────────────────────────────────────────────────────────────

fn hit_test_horizontal(pts: &[Point], cursor: Point, tol: f32) -> bool {
    pts.first()
        .map_or(false, |p| (cursor.y - p.y).abs() <= tol)
}

fn hit_test_vertical(pts: &[Point], cursor: Point, tol: f32) -> bool {
    pts.first()
        .map_or(false, |p| (cursor.x - p.x).abs() <= tol)
}

fn hit_test_line_segment(pts: &[Point], cursor: Point, tol: f32) -> bool {
    if pts.len() >= 2 {
        point_to_line_distance(cursor, pts[0], pts[1]) <= tol
    } else {
        false
    }
}

fn hit_test_ray(pts: &[Point], cursor: Point, tol: f32) -> bool {
    if pts.len() >= 2 {
        let dx = pts[1].x - pts[0].x;
        let dy = pts[1].y - pts[0].y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.0001 {
            return false;
        }
        // Distance from point to infinite line through p0-p1
        let dist = ((cursor.x - pts[0].x) * dy - (cursor.y - pts[0].y) * dx).abs() / len;
        if dist > tol {
            return false;
        }
        // Only on the forward side (past p0 in direction of p1)
        let t = ((cursor.x - pts[0].x) * dx + (cursor.y - pts[0].y) * dy) / (len * len);
        t >= -tol / len
    } else if let Some(p) = pts.first() {
        // Legacy 1-point ray: horizontal right
        (cursor.y - p.y).abs() <= tol && cursor.x >= p.x - tol
    } else {
        false
    }
}

fn hit_test_extended_line(pts: &[Point], cursor: Point, tol: f32) -> bool {
    if pts.len() >= 2 {
        let dx = pts[1].x - pts[0].x;
        let dy = pts[1].y - pts[0].y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.0001 {
            return false;
        }
        let dist =
            ((cursor.x - pts[0].x) * dy - (cursor.y - pts[0].y) * dx).abs() / len;
        dist <= tol
    } else {
        false
    }
}

// ── Shapes ───────────────────────────────────────────────────────────────

/// Hit test only the outline of a rectangle (edges). Used for VBP so that
/// clicking in the empty space inside does not select the drawing.
fn hit_test_rect_outline_only(pts: &[Point], cursor: Point, tol: f32) -> bool {
    if pts.len() < 2 {
        return false;
    }

    let min_x = pts[0].x.min(pts[1].x);
    let max_x = pts[0].x.max(pts[1].x);
    let min_y = pts[0].y.min(pts[1].y);
    let max_y = pts[0].y.max(pts[1].y);

    let near_left =
        (cursor.x - min_x).abs() <= tol && cursor.y >= min_y && cursor.y <= max_y;
    let near_right =
        (cursor.x - max_x).abs() <= tol && cursor.y >= min_y && cursor.y <= max_y;
    let near_top =
        (cursor.y - min_y).abs() <= tol && cursor.x >= min_x && cursor.x <= max_x;
    let near_bottom =
        (cursor.y - max_y).abs() <= tol && cursor.x >= min_x && cursor.x <= max_x;

    near_left || near_right || near_top || near_bottom
}

fn hit_test_rect(
    pts: &[Point],
    cursor: Point,
    tol: f32,
    style: &DrawingStyle,
) -> bool {
    if pts.len() < 2 {
        return false;
    }

    let min_x = pts[0].x.min(pts[1].x);
    let max_x = pts[0].x.max(pts[1].x);
    let min_y = pts[0].y.min(pts[1].y);
    let max_y = pts[0].y.max(pts[1].y);

    // If the rectangle has a fill, clicking inside selects it
    if style.fill_color.is_some() {
        if cursor.x >= min_x - tol
            && cursor.x <= max_x + tol
            && cursor.y >= min_y - tol
            && cursor.y <= max_y + tol
        {
            return true;
        }
    }

    // Check if near any edge
    let near_left =
        (cursor.x - min_x).abs() <= tol && cursor.y >= min_y && cursor.y <= max_y;
    let near_right =
        (cursor.x - max_x).abs() <= tol && cursor.y >= min_y && cursor.y <= max_y;
    let near_top =
        (cursor.y - min_y).abs() <= tol && cursor.x >= min_x && cursor.x <= max_x;
    let near_bottom =
        (cursor.y - max_y).abs() <= tol && cursor.x >= min_x && cursor.x <= max_x;

    near_left || near_right || near_top || near_bottom
}

fn hit_test_ellipse(
    pts: &[Point],
    cursor: Point,
    tol: f32,
    style: &DrawingStyle,
) -> bool {
    if pts.len() < 2 {
        return false;
    }

    let cx = pts[0].x;
    let cy = pts[0].y;
    let rx = (pts[1].x - cx).abs().max(1.0);
    let ry = (pts[1].y - cy).abs().max(1.0);

    // Normalized distance from center
    let nx = (cursor.x - cx) / rx;
    let ny = (cursor.y - cy) / ry;
    let d = (nx * nx + ny * ny).sqrt();

    // If filled, clicking inside selects
    if style.fill_color.is_some() && d <= 1.0 + tol / rx.min(ry) {
        return true;
    }

    // Near the boundary (d ~ 1.0)
    let norm_tolerance = tol / rx.min(ry);
    (d - 1.0).abs() <= norm_tolerance
}

// ── Fibonacci ────────────────────────────────────────────────────────────

fn hit_test_fib_retracement(
    pts: &[Point],
    cursor: Point,
    tol: f32,
    style: &DrawingStyle,
    bounds: Size,
) -> bool {
    if pts.len() < 2 {
        return false;
    }

    let config = style.fibonacci.as_ref().cloned().unwrap_or_default();

    let (min_x, max_x) = if config.extend_lines {
        (-tol, bounds.width + tol)
    } else {
        (
            pts[0].x.min(pts[1].x) - tol,
            pts[0].x.max(pts[1].x) + tol,
        )
    };

    if cursor.x < min_x || cursor.x > max_x {
        return false;
    }

    let y_range = pts[1].y - pts[0].y;

    for level in &config.levels {
        if !level.visible {
            continue;
        }
        let level_y = pts[0].y + y_range * level.ratio as f32;
        if (cursor.y - level_y).abs() <= tol {
            return true;
        }
    }
    false
}

fn hit_test_fib_extension(
    pts: &[Point],
    cursor: Point,
    tol: f32,
    style: &DrawingStyle,
) -> bool {
    if pts.len() < 3 {
        return false;
    }

    let min_x = pts.iter().map(|p| p.x).fold(f32::MAX, f32::min) - tol;
    let max_x = pts.iter().map(|p| p.x).fold(f32::MIN, f32::max) + tol;

    if cursor.x < min_x || cursor.x > max_x {
        return false;
    }

    let config = style.fibonacci.as_ref().cloned().unwrap_or_default();
    let y_range = pts[1].y - pts[0].y;

    for level in &config.levels {
        if !level.visible {
            continue;
        }
        let level_y = pts[2].y + y_range * level.ratio as f32;
        if (cursor.y - level_y).abs() <= tol {
            return true;
        }
    }
    false
}

// ── Channels ─────────────────────────────────────────────────────────────

fn hit_test_channel(pts: &[Point], cursor: Point, tol: f32) -> bool {
    if pts.len() < 3 {
        return false;
    }

    // Line 1: points[0] to points[1]
    if point_to_line_distance(cursor, pts[0], pts[1]) <= tol {
        return true;
    }

    // Line 2: parallel to line 1, passing through points[2]
    let dx = pts[1].x - pts[0].x;
    let dy = pts[1].y - pts[0].y;
    let p2_end = Point::new(pts[2].x + dx, pts[2].y + dy);
    if point_to_line_distance(cursor, pts[2], p2_end) <= tol {
        return true;
    }

    // Center line
    let center_start = Point::new(
        (pts[0].x + pts[2].x) / 2.0,
        (pts[0].y + pts[2].y) / 2.0,
    );
    let center_end = Point::new(
        (pts[1].x + pts[2].x + dx) / 2.0,
        (pts[1].y + pts[2].y + dy) / 2.0,
    );
    point_to_line_distance(cursor, center_start, center_end) <= tol
}

// ── Annotations ──────────────────────────────────────────────────────────

fn hit_test_text(pts: &[Point], cursor: Point, style: &DrawingStyle) -> bool {
    pts.first().map_or(false, |p| {
        let font_size = style.text_font_size.max(8.0);
        let text_len = style
            .text
            .as_deref()
            .map(|t| t.chars().count())
            .unwrap_or(4)
            .max(4);
        let char_width = font_size * 0.55;
        let box_w = (text_len as f32 * char_width).max(20.0) + 8.0;
        let box_h = font_size * 1.3 + 4.0;
        // Box is drawn to the right of and below p
        cursor.x >= p.x
            && cursor.x <= p.x + box_w
            && cursor.y >= p.y
            && cursor.y <= p.y + box_h
    })
}

// ── Calculators ──────────────────────────────────────────────────────────

fn hit_test_calculator(pts: &[Point], cursor: Point, tol: f32) -> bool {
    if pts.len() < 2 {
        return false;
    }

    let left_x = pts[0].x.min(pts[1].x);
    let right_x = pts[0].x.max(pts[1].x);

    // Check within horizontal bounds
    if cursor.x < left_x - tol || cursor.x > right_x + tol {
        return false;
    }

    // Check near entry line (point 0)
    let entry_y = pts[0].y;
    if (cursor.y - entry_y).abs() <= tol {
        return true;
    }

    // Check near target line (point 1)
    let target_y = pts[1].y;
    if (cursor.y - target_y).abs() <= tol {
        return true;
    }

    // Check near stop line (point 2, if exists)
    if pts.len() >= 3 {
        let stop_y = pts[2].y;
        if (cursor.y - stop_y).abs() <= tol {
            return true;
        }

        // Check near vertical edges within full height
        let min_y = entry_y.min(target_y).min(stop_y);
        let max_y = entry_y.max(target_y).max(stop_y);
        if cursor.y >= min_y && cursor.y <= max_y {
            let near_left = (cursor.x - left_x).abs() <= tol;
            let near_right = (cursor.x - right_x).abs() <= tol;
            if near_left || near_right {
                return true;
            }
        }
    }

    false
}

// ── Shared helpers ───────────────────────────────────────────────────────

/// Calculate distance from a point to a line segment.
pub fn point_to_line_distance(
    point: Point,
    line_start: Point,
    line_end: Point,
) -> f32 {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 0.0001 {
        // Line segment is essentially a point
        let px = point.x - line_start.x;
        let py = point.y - line_start.y;
        return (px * px + py * py).sqrt();
    }

    // Project point onto line, clamping to segment
    let t = ((point.x - line_start.x) * dx + (point.y - line_start.y) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    let proj_x = line_start.x + t * dx;
    let proj_y = line_start.y + t * dy;

    let px = point.x - proj_x;
    let py = point.y - proj_y;
    (px * px + py * py).sqrt()
}
