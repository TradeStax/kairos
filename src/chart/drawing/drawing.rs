//! Drawing Struct
//!
//! Represents a single drawing on the chart with its points, style, and metadata.

use super::point::DrawingPoint;
use crate::chart::ViewState;
use data::{DrawingId, DrawingStyle, DrawingTool, SerializableDrawing};
use iced::{Color, Point, Size};

/// A drawing on the chart
#[derive(Debug, Clone)]
pub struct Drawing {
    /// Unique identifier
    pub id: DrawingId,
    /// Type of drawing
    pub tool: DrawingTool,
    /// Anchor points
    pub points: Vec<DrawingPoint>,
    /// Number of user-confirmed points (excludes the preview point)
    confirmed_count: usize,
    /// Visual style
    pub style: DrawingStyle,
    /// Whether the drawing is visible
    pub visible: bool,
    /// Whether the drawing is locked (cannot be edited)
    pub locked: bool,
    /// Optional user label
    pub label: Option<String>,
}

impl Drawing {
    /// Create a new drawing with the given tool
    pub fn new(tool: DrawingTool) -> Self {
        Self {
            id: DrawingId::new(),
            tool,
            points: Vec::new(),
            confirmed_count: 0,
            style: DrawingStyle::default(),
            visible: true,
            locked: false,
            label: None,
        }
    }

    /// Create a new drawing with a specific style
    pub fn with_style(tool: DrawingTool, style: DrawingStyle) -> Self {
        Self {
            id: DrawingId::new(),
            tool,
            points: Vec::new(),
            confirmed_count: 0,
            style,
            visible: true,
            locked: false,
            label: None,
        }
    }

    /// Check if the drawing has all required confirmed points
    pub fn is_complete(&self) -> bool {
        self.confirmed_count >= self.tool.required_points()
    }

    /// Add a confirmed point to the drawing
    pub fn add_point(&mut self, point: DrawingPoint) {
        let required = self.tool.required_points();
        // Remove any preview points first
        self.points.truncate(self.confirmed_count);
        if self.confirmed_count < required {
            self.points.push(point);
            self.confirmed_count += 1;
        }
    }

    /// Update the preview point (temporary point following cursor)
    pub fn update_preview_point(&mut self, point: DrawingPoint) {
        let required = self.tool.required_points();
        // Strip any existing preview point
        self.points.truncate(self.confirmed_count);
        // Add preview if we have at least one confirmed point and need more
        if self.confirmed_count >= 1 && self.confirmed_count < required {
            self.points.push(point);
        }
    }

    /// Get the stroke color as an iced Color
    pub fn stroke_color(&self) -> Color {
        self.style.stroke_color.into()
    }

    /// Get the fill color as an iced Color (if any)
    pub fn fill_color(&self) -> Option<Color> {
        self.style.fill_color.map(|c| c.into())
    }

    /// Hit test: check if a screen point is near this drawing
    pub fn hit_test(
        &self,
        screen_point: Point,
        state: &ViewState,
        bounds: Size,
        tolerance: f32,
    ) -> bool {
        if !self.visible || self.points.is_empty() {
            return false;
        }

        let screen_points: Vec<Point> = self
            .points
            .iter()
            .map(|p| p.to_screen(state, bounds))
            .collect();

        match self.tool {
            DrawingTool::None => false,
            DrawingTool::HorizontalLine => {
                if let Some(p) = screen_points.first() {
                    (screen_point.y - p.y).abs() <= tolerance
                } else {
                    false
                }
            }
            DrawingTool::VerticalLine => {
                if let Some(p) = screen_points.first() {
                    (screen_point.x - p.x).abs() <= tolerance
                } else {
                    false
                }
            }
            DrawingTool::Line | DrawingTool::Arrow => {
                if screen_points.len() >= 2 {
                    point_to_line_distance(screen_point, screen_points[0], screen_points[1])
                        <= tolerance
                } else {
                    false
                }
            }
            DrawingTool::Ray => {
                if screen_points.len() >= 2 {
                    // 2-point ray: extends from p0 through p1
                    let dx = screen_points[1].x - screen_points[0].x;
                    let dy = screen_points[1].y - screen_points[0].y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len < 0.0001 {
                        return false;
                    }
                    // Distance from point to infinite line through p0-p1
                    let dist = ((screen_point.x - screen_points[0].x) * dy
                        - (screen_point.y - screen_points[0].y) * dx)
                        .abs()
                        / len;
                    if dist > tolerance {
                        return false;
                    }
                    // Only on the forward side (past p0 in direction of p1)
                    let t = ((screen_point.x - screen_points[0].x) * dx
                        + (screen_point.y - screen_points[0].y) * dy)
                        / (len * len);
                    t >= -tolerance / len
                } else if let Some(p) = screen_points.first() {
                    // Legacy 1-point ray: horizontal right
                    (screen_point.y - p.y).abs() <= tolerance && screen_point.x >= p.x - tolerance
                } else {
                    false
                }
            }
            DrawingTool::ExtendedLine => {
                if screen_points.len() >= 2 {
                    // Extend line infinitely in both directions
                    let dx = screen_points[1].x - screen_points[0].x;
                    let dy = screen_points[1].y - screen_points[0].y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len < 0.0001 {
                        return false;
                    }
                    // Distance from point to infinite line
                    let dist = ((screen_point.x - screen_points[0].x) * dy
                        - (screen_point.y - screen_points[0].y) * dx)
                        .abs()
                        / len;
                    dist <= tolerance
                } else {
                    false
                }
            }
            DrawingTool::Rectangle | DrawingTool::PriceRange | DrawingTool::DateRange => {
                if screen_points.len() >= 2 {
                    let min_x = screen_points[0].x.min(screen_points[1].x);
                    let max_x = screen_points[0].x.max(screen_points[1].x);
                    let min_y = screen_points[0].y.min(screen_points[1].y);
                    let max_y = screen_points[0].y.max(screen_points[1].y);

                    // Check if near any edge
                    let near_left = (screen_point.x - min_x).abs() <= tolerance
                        && screen_point.y >= min_y
                        && screen_point.y <= max_y;
                    let near_right = (screen_point.x - max_x).abs() <= tolerance
                        && screen_point.y >= min_y
                        && screen_point.y <= max_y;
                    let near_top = (screen_point.y - min_y).abs() <= tolerance
                        && screen_point.x >= min_x
                        && screen_point.x <= max_x;
                    let near_bottom = (screen_point.y - max_y).abs() <= tolerance
                        && screen_point.x >= min_x
                        && screen_point.x <= max_x;

                    near_left || near_right || near_top || near_bottom
                } else {
                    false
                }
            }
            DrawingTool::Ellipse => {
                if screen_points.len() >= 2 {
                    let cx = screen_points[0].x;
                    let cy = screen_points[0].y;
                    let rx = (screen_points[1].x - cx).abs().max(1.0);
                    let ry = (screen_points[1].y - cy).abs().max(1.0);

                    // Normalized distance from center
                    let nx = (screen_point.x - cx) / rx;
                    let ny = (screen_point.y - cy) / ry;
                    let d = (nx * nx + ny * ny).sqrt();

                    // Near the boundary (d ~ 1.0)
                    let norm_tolerance = tolerance / rx.min(ry);
                    (d - 1.0).abs() <= norm_tolerance
                } else {
                    false
                }
            }
            DrawingTool::FibRetracement => {
                if screen_points.len() >= 2 {
                    let min_x = screen_points[0].x.min(screen_points[1].x) - tolerance;
                    let max_x = screen_points[0].x.max(screen_points[1].x) + tolerance;

                    // Check if within horizontal bounds
                    if screen_point.x < min_x || screen_point.x > max_x {
                        return false;
                    }

                    let config = self.style.fibonacci.as_ref().cloned().unwrap_or_default();
                    let y_range = screen_points[1].y - screen_points[0].y;

                    // Test each visible level line
                    for level in &config.levels {
                        if !level.visible {
                            continue;
                        }
                        let level_y = screen_points[0].y + y_range * level.ratio as f32;
                        if (screen_point.y - level_y).abs() <= tolerance {
                            return true;
                        }
                    }
                    false
                } else {
                    false
                }
            }
            DrawingTool::FibExtension => {
                if screen_points.len() >= 3 {
                    let min_x =
                        screen_points.iter().map(|p| p.x).fold(f32::MAX, f32::min) - tolerance;
                    let max_x =
                        screen_points.iter().map(|p| p.x).fold(f32::MIN, f32::max) + tolerance;

                    if screen_point.x < min_x || screen_point.x > max_x {
                        return false;
                    }

                    let config = self.style.fibonacci.as_ref().cloned().unwrap_or_default();
                    let y_range = screen_points[1].y - screen_points[0].y;

                    for level in &config.levels {
                        if !level.visible {
                            continue;
                        }
                        let level_y = screen_points[2].y + y_range * level.ratio as f32;
                        if (screen_point.y - level_y).abs() <= tolerance {
                            return true;
                        }
                    }
                    false
                } else {
                    false
                }
            }
            DrawingTool::ParallelChannel => {
                if screen_points.len() >= 3 {
                    // Line 1: points[0] to points[1]
                    let d1 =
                        point_to_line_distance(screen_point, screen_points[0], screen_points[1]);
                    if d1 <= tolerance {
                        return true;
                    }

                    // Line 2: parallel to line 1, passing through points[2]
                    let dx = screen_points[1].x - screen_points[0].x;
                    let dy = screen_points[1].y - screen_points[0].y;
                    let p2_start = screen_points[2];
                    let p2_end = Point::new(screen_points[2].x + dx, screen_points[2].y + dy);
                    let d2 = point_to_line_distance(screen_point, p2_start, p2_end);
                    if d2 <= tolerance {
                        return true;
                    }

                    // Center line
                    let center_start = Point::new(
                        (screen_points[0].x + screen_points[2].x) / 2.0,
                        (screen_points[0].y + screen_points[2].y) / 2.0,
                    );
                    let center_end = Point::new(
                        (screen_points[1].x + screen_points[2].x + dx) / 2.0,
                        (screen_points[1].y + screen_points[2].y + dy) / 2.0,
                    );
                    let d3 = point_to_line_distance(screen_point, center_start, center_end);
                    d3 <= tolerance
                } else {
                    false
                }
            }
            DrawingTool::TextLabel | DrawingTool::PriceLabel => {
                if let Some(p) = screen_points.first() {
                    // Simple bounding box (approximate text area)
                    let half_w = 50.0;
                    let half_h = 12.0;
                    (screen_point.x - p.x).abs() <= half_w && (screen_point.y - p.y).abs() <= half_h
                } else {
                    false
                }
            }
        }
    }

    /// Get the handle positions for selection (in screen coordinates)
    pub fn handle_positions(&self, state: &ViewState, bounds: Size) -> Vec<Point> {
        self.points
            .iter()
            .map(|p| p.to_screen(state, bounds))
            .collect()
    }

    /// Check if a screen point is near a handle, returns handle index if so
    pub fn hit_test_handle(
        &self,
        screen_point: Point,
        state: &ViewState,
        bounds: Size,
        handle_size: f32,
    ) -> Option<usize> {
        if self.locked {
            return None;
        }

        let handles = self.handle_positions(state, bounds);
        let half_size = handle_size / 2.0;

        for (i, handle) in handles.iter().enumerate() {
            if (screen_point.x - handle.x).abs() <= half_size
                && (screen_point.y - handle.y).abs() <= half_size
            {
                return Some(i);
            }
        }
        None
    }

    /// Convert to serializable format
    pub fn to_serializable(&self) -> SerializableDrawing {
        SerializableDrawing {
            id: self.id,
            tool: self.tool,
            points: self.points.iter().map(|p| p.to_serializable()).collect(),
            style: self.style.clone(),
            visible: self.visible,
            locked: self.locked,
            label: self.label.clone(),
        }
    }

    /// Create from serializable format
    pub fn from_serializable(drawing: &SerializableDrawing) -> Self {
        let points: Vec<DrawingPoint> = drawing.points.iter().map(DrawingPoint::from).collect();
        let confirmed_count = points.len();
        Self {
            id: drawing.id,
            tool: drawing.tool,
            points,
            confirmed_count,
            style: drawing.style.clone(),
            visible: drawing.visible,
            locked: drawing.locked,
            label: drawing.label.clone(),
        }
    }
}

/// Calculate distance from a point to a line segment
fn point_to_line_distance(point: Point, line_start: Point, line_end: Point) -> f32 {
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

impl From<&SerializableDrawing> for Drawing {
    fn from(drawing: &SerializableDrawing) -> Self {
        Drawing::from_serializable(drawing)
    }
}

impl From<Drawing> for SerializableDrawing {
    fn from(drawing: Drawing) -> Self {
        drawing.to_serializable()
    }
}
