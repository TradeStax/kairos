//! Per-tool hit testing
//!
//! Each function performs geometric distance testing for a specific drawing
//! tool category. The main entry point is [`hit_test_tool`], which dispatches
//! to the appropriate function based on `DrawingTool`.

use crate::drawing::{DrawingStyle, DrawingTool};
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
        DrawingTool::ExtendedLine => hit_test_extended_line(screen_points, cursor, tolerance),
        DrawingTool::Rectangle => hit_test_rect(screen_points, cursor, tolerance, style),
        DrawingTool::Ellipse => hit_test_ellipse(screen_points, cursor, tolerance, style),
        DrawingTool::FibRetracement => {
            hit_test_fib_retracement(screen_points, cursor, tolerance, style, bounds)
        }
        DrawingTool::FibExtension => {
            hit_test_fib_extension(screen_points, cursor, tolerance, style)
        }
        DrawingTool::ParallelChannel => hit_test_channel(screen_points, cursor, tolerance),
        DrawingTool::TextLabel | DrawingTool::PriceLabel => {
            hit_test_text(screen_points, cursor, style)
        }
        DrawingTool::BuyCalculator | DrawingTool::SellCalculator => {
            hit_test_calculator(screen_points, cursor, tolerance)
        }
        DrawingTool::VolumeProfile | DrawingTool::DeltaProfile => {
            hit_test_rect_outline_only(screen_points, cursor, tolerance)
        }
        DrawingTool::AiContext => hit_test_rect(screen_points, cursor, tolerance, style),
    }
}

// ── Lines ────────────────────────────────────────────────────────────────

fn hit_test_horizontal(pts: &[Point], cursor: Point, tol: f32) -> bool {
    pts.first().is_some_and(|p| (cursor.y - p.y).abs() <= tol)
}

fn hit_test_vertical(pts: &[Point], cursor: Point, tol: f32) -> bool {
    pts.first().is_some_and(|p| (cursor.x - p.x).abs() <= tol)
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
        // 1-point ray: horizontal right
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
        let dist = ((cursor.x - pts[0].x) * dy - (cursor.y - pts[0].y) * dx).abs() / len;
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

    let near_left = (cursor.x - min_x).abs() <= tol && cursor.y >= min_y && cursor.y <= max_y;
    let near_right = (cursor.x - max_x).abs() <= tol && cursor.y >= min_y && cursor.y <= max_y;
    let near_top = (cursor.y - min_y).abs() <= tol && cursor.x >= min_x && cursor.x <= max_x;
    let near_bottom = (cursor.y - max_y).abs() <= tol && cursor.x >= min_x && cursor.x <= max_x;

    near_left || near_right || near_top || near_bottom
}

fn hit_test_rect(pts: &[Point], cursor: Point, tol: f32, style: &DrawingStyle) -> bool {
    if pts.len() < 2 {
        return false;
    }

    let min_x = pts[0].x.min(pts[1].x);
    let max_x = pts[0].x.max(pts[1].x);
    let min_y = pts[0].y.min(pts[1].y);
    let max_y = pts[0].y.max(pts[1].y);

    // If the rectangle has a fill, clicking inside selects it
    if style.fill_color.is_some()
        && cursor.x >= min_x - tol
        && cursor.x <= max_x + tol
        && cursor.y >= min_y - tol
        && cursor.y <= max_y + tol
    {
        return true;
    }

    // Check if near any edge
    let near_left = (cursor.x - min_x).abs() <= tol && cursor.y >= min_y && cursor.y <= max_y;
    let near_right = (cursor.x - max_x).abs() <= tol && cursor.y >= min_y && cursor.y <= max_y;
    let near_top = (cursor.y - min_y).abs() <= tol && cursor.x >= min_x && cursor.x <= max_x;
    let near_bottom = (cursor.y - max_y).abs() <= tol && cursor.x >= min_x && cursor.x <= max_x;

    near_left || near_right || near_top || near_bottom
}

fn hit_test_ellipse(pts: &[Point], cursor: Point, tol: f32, style: &DrawingStyle) -> bool {
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
        (pts[0].x.min(pts[1].x) - tol, pts[0].x.max(pts[1].x) + tol)
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

fn hit_test_fib_extension(pts: &[Point], cursor: Point, tol: f32, style: &DrawingStyle) -> bool {
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
    let center_start = Point::new((pts[0].x + pts[2].x) / 2.0, (pts[0].y + pts[2].y) / 2.0);
    let center_end = Point::new(
        (pts[1].x + pts[2].x + dx) / 2.0,
        (pts[1].y + pts[2].y + dy) / 2.0,
    );
    point_to_line_distance(cursor, center_start, center_end) <= tol
}

// ── Annotations ──────────────────────────────────────────────────────────

fn hit_test_text(pts: &[Point], cursor: Point, style: &DrawingStyle) -> bool {
    pts.first().is_some_and(|p| {
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
        cursor.x >= p.x && cursor.x <= p.x + box_w && cursor.y >= p.y && cursor.y <= p.y + box_h
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
pub fn point_to_line_distance(point: Point, line_start: Point, line_end: Point) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    fn default_style() -> DrawingStyle {
        DrawingStyle::default()
    }

    fn filled_style() -> DrawingStyle {
        DrawingStyle {
            fill_color: Some(data::SerializableColor::new(1.0, 0.0, 0.0, 1.0)),
            ..DrawingStyle::default()
        }
    }

    fn fib_style(extend: bool) -> DrawingStyle {
        DrawingStyle {
            fibonacci: Some(crate::drawing::FibonacciConfig {
                extend_lines: extend,
                ..Default::default()
            }),
            ..DrawingStyle::default()
        }
    }

    const TOL: f32 = 5.0;

    // ── point_to_line_distance ──────────────────────────────────────

    #[test]
    fn ptld_point_on_segment_start() {
        let d = point_to_line_distance(pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 0.0));
        assert!(d < 0.001);
    }

    #[test]
    fn ptld_point_on_segment_end() {
        let d = point_to_line_distance(pt(10.0, 0.0), pt(0.0, 0.0), pt(10.0, 0.0));
        assert!(d < 0.001);
    }

    #[test]
    fn ptld_point_on_segment_midpoint() {
        let d = point_to_line_distance(pt(5.0, 0.0), pt(0.0, 0.0), pt(10.0, 0.0));
        assert!(d < 0.001);
    }

    #[test]
    fn ptld_point_perpendicular() {
        let d = point_to_line_distance(pt(5.0, 3.0), pt(0.0, 0.0), pt(10.0, 0.0));
        assert!((d - 3.0).abs() < 0.001);
    }

    #[test]
    fn ptld_point_beyond_end() {
        // closest to end (10,0), distance = sqrt(5^2 + 3^2) = sqrt(34)
        let d = point_to_line_distance(pt(15.0, 3.0), pt(0.0, 0.0), pt(10.0, 0.0));
        let expected = (25.0_f32 + 9.0).sqrt();
        assert!((d - expected).abs() < 0.01);
    }

    #[test]
    fn ptld_point_before_start() {
        // closest to start (0,0)
        let d = point_to_line_distance(pt(-3.0, 4.0), pt(0.0, 0.0), pt(10.0, 0.0));
        let expected = (9.0_f32 + 16.0).sqrt(); // 5.0
        assert!((d - expected).abs() < 0.01);
    }

    #[test]
    fn ptld_degenerate_zero_length() {
        let d = point_to_line_distance(pt(3.0, 4.0), pt(5.0, 5.0), pt(5.0, 5.0));
        let expected = ((3.0 - 5.0_f32).powi(2) + (4.0 - 5.0_f32).powi(2)).sqrt();
        assert!((d - expected).abs() < 0.01);
    }

    #[test]
    fn ptld_diagonal_line() {
        // line from (0,0) to (10,10), point at (0,10) — distance = 10/sqrt(2)
        let d = point_to_line_distance(pt(0.0, 10.0), pt(0.0, 0.0), pt(10.0, 10.0));
        let expected = 10.0 / 2.0_f32.sqrt();
        assert!((d - expected).abs() < 0.1);
    }

    // ── hit_test_horizontal ─────────────────────────────────────────

    #[test]
    fn horizontal_hit() {
        assert!(hit_test_horizontal(
            &[pt(50.0, 100.0)],
            pt(200.0, 103.0),
            TOL
        ));
    }

    #[test]
    fn horizontal_miss() {
        assert!(!hit_test_horizontal(
            &[pt(50.0, 100.0)],
            pt(200.0, 106.0),
            TOL
        ));
    }

    #[test]
    fn horizontal_empty_pts() {
        assert!(!hit_test_horizontal(&[], pt(200.0, 100.0), TOL));
    }

    #[test]
    fn horizontal_exact() {
        assert!(hit_test_horizontal(
            &[pt(50.0, 100.0)],
            pt(999.0, 100.0),
            TOL
        ));
    }

    // ── hit_test_vertical ───────────────────────────────────────────

    #[test]
    fn vertical_hit() {
        assert!(hit_test_vertical(&[pt(100.0, 50.0)], pt(103.0, 500.0), TOL));
    }

    #[test]
    fn vertical_miss() {
        assert!(!hit_test_vertical(
            &[pt(100.0, 50.0)],
            pt(106.0, 500.0),
            TOL
        ));
    }

    #[test]
    fn vertical_empty_pts() {
        assert!(!hit_test_vertical(&[], pt(100.0, 100.0), TOL));
    }

    // ── hit_test_line_segment ───────────────────────────────────────

    #[test]
    fn line_segment_hit_midpoint() {
        let pts = [pt(0.0, 0.0), pt(100.0, 0.0)];
        assert!(hit_test_line_segment(&pts, pt(50.0, 3.0), TOL));
    }

    #[test]
    fn line_segment_miss() {
        let pts = [pt(0.0, 0.0), pt(100.0, 0.0)];
        assert!(!hit_test_line_segment(&pts, pt(50.0, 10.0), TOL));
    }

    #[test]
    fn line_segment_single_point() {
        assert!(!hit_test_line_segment(&[pt(0.0, 0.0)], pt(0.0, 0.0), TOL));
    }

    #[test]
    fn line_segment_empty() {
        assert!(!hit_test_line_segment(&[], pt(0.0, 0.0), TOL));
    }

    #[test]
    fn line_segment_diagonal_hit() {
        let pts = [pt(0.0, 0.0), pt(100.0, 100.0)];
        // point on the line
        assert!(hit_test_line_segment(&pts, pt(50.0, 50.0), TOL));
    }

    // ── hit_test_ray ────────────────────────────────────────────────

    #[test]
    fn ray_forward_hit() {
        let pts = [pt(0.0, 0.0), pt(10.0, 0.0)];
        // far along the ray direction — should hit
        assert!(hit_test_ray(&pts, pt(500.0, 2.0), TOL));
    }

    #[test]
    fn ray_behind_miss() {
        let pts = [pt(100.0, 100.0), pt(200.0, 100.0)];
        // behind the ray origin — should miss
        assert!(!hit_test_ray(&pts, pt(0.0, 100.0), TOL));
    }

    #[test]
    fn ray_perpendicular_miss() {
        let pts = [pt(0.0, 0.0), pt(10.0, 0.0)];
        assert!(!hit_test_ray(&pts, pt(50.0, 20.0), TOL));
    }

    #[test]
    fn ray_degenerate_coincident_points() {
        let pts = [pt(5.0, 5.0), pt(5.0, 5.0)];
        assert!(!hit_test_ray(&pts, pt(5.0, 5.0), TOL));
    }

    #[test]
    fn ray_single_point() {
        // 1-point ray: horizontal right
        let pts = [pt(50.0, 100.0)];
        assert!(hit_test_ray(&pts, pt(200.0, 103.0), TOL));
        assert!(!hit_test_ray(&pts, pt(30.0, 100.0), TOL)); // behind
    }

    #[test]
    fn ray_empty() {
        assert!(!hit_test_ray(&[], pt(0.0, 0.0), TOL));
    }

    // ── hit_test_extended_line ──────────────────────────────────────

    #[test]
    fn extended_line_both_directions() {
        let pts = [pt(100.0, 100.0), pt(200.0, 100.0)];
        // far to the left (before p0) — should still hit for infinite line
        assert!(hit_test_extended_line(&pts, pt(-500.0, 102.0), TOL));
        // far to the right
        assert!(hit_test_extended_line(&pts, pt(999.0, 98.0), TOL));
    }

    #[test]
    fn extended_line_miss_perpendicular() {
        let pts = [pt(100.0, 100.0), pt(200.0, 100.0)];
        assert!(!hit_test_extended_line(&pts, pt(150.0, 120.0), TOL));
    }

    #[test]
    fn extended_line_degenerate() {
        let pts = [pt(5.0, 5.0), pt(5.0, 5.0)];
        assert!(!hit_test_extended_line(&pts, pt(5.0, 5.0), TOL));
    }

    #[test]
    fn extended_line_insufficient_points() {
        assert!(!hit_test_extended_line(&[pt(0.0, 0.0)], pt(0.0, 0.0), TOL));
    }

    // ── hit_test_rect_outline_only ──────────────────────────────────

    #[test]
    fn rect_outline_left_edge() {
        let pts = [pt(10.0, 10.0), pt(100.0, 100.0)];
        assert!(hit_test_rect_outline_only(&pts, pt(12.0, 50.0), TOL));
    }

    #[test]
    fn rect_outline_inside_miss() {
        let pts = [pt(10.0, 10.0), pt(100.0, 100.0)];
        assert!(!hit_test_rect_outline_only(&pts, pt(50.0, 50.0), TOL));
    }

    #[test]
    fn rect_outline_top_edge() {
        let pts = [pt(10.0, 10.0), pt(100.0, 100.0)];
        assert!(hit_test_rect_outline_only(&pts, pt(50.0, 12.0), TOL));
    }

    #[test]
    fn rect_outline_insufficient_points() {
        assert!(!hit_test_rect_outline_only(
            &[pt(0.0, 0.0)],
            pt(0.0, 0.0),
            TOL
        ));
    }

    // ── hit_test_rect ───────────────────────────────────────────────

    #[test]
    fn rect_no_fill_edge_hit() {
        let pts = [pt(10.0, 10.0), pt(100.0, 100.0)];
        let style = default_style();
        assert!(hit_test_rect(&pts, pt(12.0, 50.0), TOL, &style));
    }

    #[test]
    fn rect_no_fill_center_miss() {
        let pts = [pt(10.0, 10.0), pt(100.0, 100.0)];
        let style = default_style();
        assert!(!hit_test_rect(&pts, pt(50.0, 50.0), TOL, &style));
    }

    #[test]
    fn rect_filled_center_hit() {
        let pts = [pt(10.0, 10.0), pt(100.0, 100.0)];
        let style = filled_style();
        assert!(hit_test_rect(&pts, pt(50.0, 50.0), TOL, &style));
    }

    #[test]
    fn rect_filled_outside_miss() {
        let pts = [pt(10.0, 10.0), pt(100.0, 100.0)];
        let style = filled_style();
        assert!(!hit_test_rect(&pts, pt(200.0, 200.0), TOL, &style));
    }

    #[test]
    fn rect_reversed_points() {
        // points given in reverse order (bottom-right, top-left)
        let pts = [pt(100.0, 100.0), pt(10.0, 10.0)];
        let style = filled_style();
        assert!(hit_test_rect(&pts, pt(50.0, 50.0), TOL, &style));
    }

    // ── hit_test_ellipse ────────────────────────────────────────────

    #[test]
    fn ellipse_on_boundary() {
        // center (50,50), radius point (100,50) → rx=50, ry=1 (clamped)
        // Better test: center (50,50), radius point (100,100) → rx=50, ry=50
        let pts = [pt(50.0, 50.0), pt(100.0, 100.0)];
        let style = default_style();
        // Point on boundary at (100, 50): nx=(100-50)/50=1, ny=0, d=1
        assert!(hit_test_ellipse(&pts, pt(100.0, 50.0), TOL, &style));
    }

    #[test]
    fn ellipse_inside_no_fill_miss() {
        let pts = [pt(50.0, 50.0), pt(100.0, 100.0)];
        let style = default_style();
        // well inside
        assert!(!hit_test_ellipse(&pts, pt(50.0, 50.0), TOL, &style));
    }

    #[test]
    fn ellipse_filled_inside_hit() {
        let pts = [pt(50.0, 50.0), pt(100.0, 100.0)];
        let style = filled_style();
        assert!(hit_test_ellipse(&pts, pt(50.0, 50.0), TOL, &style));
    }

    #[test]
    fn ellipse_outside_miss() {
        let pts = [pt(50.0, 50.0), pt(100.0, 100.0)];
        let style = default_style();
        assert!(!hit_test_ellipse(&pts, pt(200.0, 200.0), TOL, &style));
    }

    #[test]
    fn ellipse_insufficient_points() {
        assert!(!hit_test_ellipse(
            &[pt(50.0, 50.0)],
            pt(50.0, 50.0),
            TOL,
            &default_style()
        ));
    }

    // ── hit_test_fib_retracement ────────────────────────────────────

    #[test]
    fn fib_retracement_hit_on_level() {
        let pts = [pt(50.0, 0.0), pt(150.0, 100.0)];
        let style = fib_style(false);
        let bounds = Size::new(800.0, 600.0);
        // Level 0.0 is at y=0, level 0.5 is at y=50, level 1.0 is at y=100
        assert!(hit_test_fib_retracement(
            &pts,
            pt(100.0, 50.0),
            TOL,
            &style,
            bounds
        ));
    }

    #[test]
    fn fib_retracement_miss_between_levels() {
        let pts = [pt(50.0, 0.0), pt(150.0, 100.0)];
        let style = fib_style(false);
        let bounds = Size::new(800.0, 600.0);
        // y=30 is between 23.6% (23.6) and 38.2% (38.2) — might miss both
        assert!(!hit_test_fib_retracement(
            &pts,
            pt(100.0, 30.0),
            TOL,
            &style,
            bounds
        ));
    }

    #[test]
    fn fib_retracement_outside_x_range() {
        let pts = [pt(50.0, 0.0), pt(150.0, 100.0)];
        let style = fib_style(false);
        let bounds = Size::new(800.0, 600.0);
        // x=200 is beyond the x range of the drawing
        assert!(!hit_test_fib_retracement(
            &pts,
            pt(200.0, 50.0),
            TOL,
            &style,
            bounds
        ));
    }

    #[test]
    fn fib_retracement_extended_lines_far_x() {
        let pts = [pt(50.0, 0.0), pt(150.0, 100.0)];
        let style = fib_style(true);
        let bounds = Size::new(800.0, 600.0);
        // extended lines: x=400 should still be valid
        assert!(hit_test_fib_retracement(
            &pts,
            pt(400.0, 50.0),
            TOL,
            &style,
            bounds
        ));
    }

    #[test]
    fn fib_retracement_insufficient_points() {
        let style = fib_style(false);
        let bounds = Size::new(800.0, 600.0);
        assert!(!hit_test_fib_retracement(
            &[pt(50.0, 0.0)],
            pt(50.0, 0.0),
            TOL,
            &style,
            bounds
        ));
    }

    // ── hit_test_fib_extension ──────────────────────────────────────

    #[test]
    fn fib_extension_hit_on_level() {
        // 3 points: p0, p1 define range; p2 is base for extension
        let pts = [pt(50.0, 0.0), pt(150.0, 100.0), pt(100.0, 200.0)];
        let style = fib_style(false);
        // level 0.0 at y = 200 + (100-0)*0 = 200
        assert!(hit_test_fib_extension(&pts, pt(100.0, 200.0), TOL, &style));
    }

    #[test]
    fn fib_extension_miss() {
        let pts = [pt(50.0, 0.0), pt(150.0, 100.0), pt(100.0, 200.0)];
        let style = fib_style(false);
        assert!(!hit_test_fib_extension(&pts, pt(100.0, 150.0), TOL, &style));
    }

    #[test]
    fn fib_extension_insufficient_points() {
        let style = fib_style(false);
        assert!(!hit_test_fib_extension(
            &[pt(0.0, 0.0), pt(10.0, 10.0)],
            pt(5.0, 5.0),
            TOL,
            &style
        ));
    }

    // ── hit_test_channel ────────────────────────────────────────────

    #[test]
    fn channel_hit_first_line() {
        let pts = [pt(0.0, 0.0), pt(100.0, 0.0), pt(0.0, 50.0)];
        // near line 1 (y=0)
        assert!(hit_test_channel(&pts, pt(50.0, 3.0), TOL));
    }

    #[test]
    fn channel_hit_second_line() {
        let pts = [pt(0.0, 0.0), pt(100.0, 0.0), pt(0.0, 50.0)];
        // line 2 is parallel to line 1, through pt(0,50) → y=50
        assert!(hit_test_channel(&pts, pt(50.0, 52.0), TOL));
    }

    #[test]
    fn channel_hit_center_line() {
        let pts = [pt(0.0, 0.0), pt(100.0, 0.0), pt(0.0, 50.0)];
        // center line at y=25
        assert!(hit_test_channel(&pts, pt(50.0, 27.0), TOL));
    }

    #[test]
    fn channel_miss_outside() {
        let pts = [pt(0.0, 0.0), pt(100.0, 0.0), pt(0.0, 50.0)];
        assert!(!hit_test_channel(&pts, pt(50.0, 80.0), TOL));
    }

    #[test]
    fn channel_insufficient_points() {
        assert!(!hit_test_channel(
            &[pt(0.0, 0.0), pt(10.0, 10.0)],
            pt(5.0, 5.0),
            TOL
        ));
    }

    // ── hit_test_text ───────────────────────────────────────────────

    #[test]
    fn text_inside_box() {
        let pts = [pt(10.0, 10.0)];
        let style = DrawingStyle {
            text: Some("Hello".into()),
            text_font_size: 13.0,
            ..default_style()
        };
        // box starts at (10, 10), extends right and down
        assert!(hit_test_text(&pts, pt(15.0, 15.0), &style));
    }

    #[test]
    fn text_outside_box() {
        let pts = [pt(10.0, 10.0)];
        let style = DrawingStyle {
            text: Some("Hi".into()),
            text_font_size: 13.0,
            ..default_style()
        };
        // far below/right of the small text box
        assert!(!hit_test_text(&pts, pt(200.0, 200.0), &style));
    }

    #[test]
    fn text_empty_pts() {
        assert!(!hit_test_text(&[], pt(10.0, 10.0), &default_style()));
    }

    // ── hit_test_calculator ─────────────────────────────────────────

    #[test]
    fn calculator_hit_entry_line() {
        let pts = [pt(50.0, 100.0), pt(150.0, 200.0)];
        // near entry y=100
        assert!(hit_test_calculator(&pts, pt(100.0, 102.0), TOL));
    }

    #[test]
    fn calculator_hit_target_line() {
        let pts = [pt(50.0, 100.0), pt(150.0, 200.0)];
        // near target y=200
        assert!(hit_test_calculator(&pts, pt(100.0, 198.0), TOL));
    }

    #[test]
    fn calculator_miss_between_lines() {
        let pts = [pt(50.0, 100.0), pt(150.0, 200.0)];
        // y=150 between entry and target, not near either
        assert!(!hit_test_calculator(&pts, pt(100.0, 150.0), TOL));
    }

    #[test]
    fn calculator_hit_stop_line() {
        let pts = [pt(50.0, 100.0), pt(150.0, 200.0), pt(100.0, 50.0)];
        // near stop y=50
        assert!(hit_test_calculator(&pts, pt(100.0, 52.0), TOL));
    }

    #[test]
    fn calculator_hit_vertical_edge() {
        let pts = [pt(50.0, 100.0), pt(150.0, 200.0), pt(100.0, 50.0)];
        // near left edge x=50, within y range
        assert!(hit_test_calculator(&pts, pt(52.0, 80.0), TOL));
    }

    #[test]
    fn calculator_outside_x_range() {
        let pts = [pt(50.0, 100.0), pt(150.0, 200.0)];
        assert!(!hit_test_calculator(&pts, pt(200.0, 100.0), TOL));
    }

    #[test]
    fn calculator_insufficient_points() {
        assert!(!hit_test_calculator(
            &[pt(50.0, 100.0)],
            pt(50.0, 100.0),
            TOL
        ));
    }

    // ── hit_test_tool dispatch ──────────────────────────────────────

    #[test]
    fn dispatch_none_always_false() {
        assert!(!hit_test_tool(
            DrawingTool::None,
            &[pt(0.0, 0.0)],
            pt(0.0, 0.0),
            TOL,
            &default_style(),
            Size::new(800.0, 600.0),
        ));
    }

    #[test]
    fn dispatch_horizontal_line() {
        assert!(hit_test_tool(
            DrawingTool::HorizontalLine,
            &[pt(50.0, 100.0)],
            pt(200.0, 102.0),
            TOL,
            &default_style(),
            Size::new(800.0, 600.0),
        ));
    }

    #[test]
    fn dispatch_arrow_uses_line_segment() {
        let pts = [pt(0.0, 0.0), pt(100.0, 0.0)];
        assert!(hit_test_tool(
            DrawingTool::Arrow,
            &pts,
            pt(50.0, 3.0),
            TOL,
            &default_style(),
            Size::new(800.0, 600.0),
        ));
    }

    #[test]
    fn dispatch_volume_profile_outline_only() {
        let pts = [pt(10.0, 10.0), pt(100.0, 100.0)];
        // inside the box but not near edge — should miss (outline only)
        assert!(!hit_test_tool(
            DrawingTool::VolumeProfile,
            &pts,
            pt(50.0, 50.0),
            TOL,
            &default_style(),
            Size::new(800.0, 600.0),
        ));
    }

    // ── Negative / large coordinates ────────────────────────────────

    #[test]
    fn negative_coordinates_line_hit() {
        let pts = [pt(-100.0, -100.0), pt(-50.0, -50.0)];
        assert!(hit_test_line_segment(&pts, pt(-75.0, -75.0), TOL));
    }

    #[test]
    fn large_coordinates_horizontal() {
        let pts = [pt(1e6, 1e6)];
        assert!(hit_test_horizontal(&pts, pt(0.0, 1e6 + 2.0), TOL));
    }
}
