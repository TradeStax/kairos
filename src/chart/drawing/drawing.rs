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
            style,
            visible: true,
            locked: false,
            label: None,
        }
    }

    /// Check if the drawing has all required points
    pub fn is_complete(&self) -> bool {
        self.points.len() >= self.tool.required_points()
    }

    /// Add a point to the drawing
    pub fn add_point(&mut self, point: DrawingPoint) {
        let required = self.tool.required_points();
        if self.points.len() < required {
            self.points.push(point);
        }
    }

    /// Update the preview point (last point while drawing)
    pub fn update_preview_point(&mut self, point: DrawingPoint) {
        let required = self.tool.required_points();
        if required > 1 && self.points.len() == required {
            // Replace the last point with the preview
            self.points.pop();
            self.points.push(point);
        } else if self.points.len() == required - 1 {
            // Add the preview point
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
            DrawingTool::Line | DrawingTool::TrendLine => {
                if screen_points.len() >= 2 {
                    point_to_line_distance(screen_point, screen_points[0], screen_points[1])
                        <= tolerance
                } else {
                    false
                }
            }
            DrawingTool::Ray => {
                if screen_points.len() >= 2 {
                    point_to_ray_distance(screen_point, screen_points[0], screen_points[1], bounds)
                        <= tolerance
                } else {
                    false
                }
            }
            DrawingTool::Rectangle => {
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
        Self {
            id: drawing.id,
            tool: drawing.tool,
            points: drawing.points.iter().map(DrawingPoint::from).collect(),
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

/// Calculate distance from a point to a ray
fn point_to_ray_distance(point: Point, ray_start: Point, ray_direction_point: Point, bounds: Size) -> f32 {
    let dx = ray_direction_point.x - ray_start.x;
    let dy = ray_direction_point.y - ray_start.y;
    let len = (dx * dx + dy * dy).sqrt();

    if len < 0.0001 {
        let px = point.x - ray_start.x;
        let py = point.y - ray_start.y;
        return (px * px + py * py).sqrt();
    }

    // Normalize direction
    let dir_x = dx / len;
    let dir_y = dy / len;

    // Extend ray to bounds edge
    let max_extent = (bounds.width.max(bounds.height)) * 2.0;
    let ray_end = Point::new(
        ray_start.x + dir_x * max_extent,
        ray_start.y + dir_y * max_extent,
    );

    // Now treat as line segment from ray_start to ray_end
    point_to_line_distance(point, ray_start, ray_end)
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
