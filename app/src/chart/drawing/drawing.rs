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
        crate::style::theme_bridge::rgba_to_iced_color(self.style.stroke_color)
    }

    /// Get the fill color as an iced Color (if any)
    pub fn fill_color(&self) -> Option<Color> {
        self.style
            .fill_color
            .map(crate::style::theme_bridge::rgba_to_iced_color)
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
            .map(|p| p.as_screen_point(state, bounds))
            .collect();

        super::hit_test::hit_test_tool(
            self.tool,
            &screen_points,
            screen_point,
            tolerance,
            &self.style,
            bounds,
        )
    }

    /// Get the handle positions for selection (in screen coordinates)
    pub fn handle_positions(&self, state: &ViewState, bounds: Size) -> Vec<Point> {
        self.points
            .iter()
            .map(|p| p.as_screen_point(state, bounds))
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

    /// Create a clone of this drawing with a fresh unique ID.
    pub fn clone_with_new_id(&self) -> Self {
        Self {
            id: DrawingId::new(),
            ..self.clone()
        }
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
