//! Drawing Struct
//!
//! Represents a single drawing on the chart with its points, style, and metadata.

use super::point::DrawingPoint;
use crate::chart::ViewState;
use data::{DrawingId, DrawingStyle, DrawingTool, SerializableDrawing};
use exchange::util::Price as ExchangePrice;
use iced::{Color, Point, Size};

/// A drawing on the chart
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
    /// Embedded VBP study instance (only for VolumeProfile drawings).
    pub(crate) vbp_study: Option<Box<study::orderflow::VbpStudy>>,
    /// Cached open prices at left/right edge candles (transient, not serialized).
    pub(crate) vbp_edge_opens: Option<(ExchangePrice, ExchangePrice)>,
}

impl std::fmt::Debug for Drawing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Drawing")
            .field("id", &self.id)
            .field("tool", &self.tool)
            .field("points", &self.points)
            .field("visible", &self.visible)
            .field("locked", &self.locked)
            .field("vbp_study", &self.vbp_study.is_some())
            .finish()
    }
}

impl Clone for Drawing {
    fn clone(&self) -> Self {
        let vbp_study = self.vbp_study.as_ref().map(|s| {
            // Reconstruct from config + anchor points
            let (start, end) = if self.points.len() >= 2 {
                let t1 = self.points[0].time;
                let t2 = self.points[1].time;
                (t1.min(t2), t1.max(t2))
            } else {
                (0, 0)
            };
            let mut cloned =
                study::orderflow::VbpStudy::for_range(start, end);
            let exported = s.export_config();
            cloned.import_config(&exported);
            Box::new(cloned)
        });
        Self {
            id: self.id,
            tool: self.tool,
            points: self.points.clone(),
            confirmed_count: self.confirmed_count,
            style: self.style.clone(),
            visible: self.visible,
            locked: self.locked,
            label: self.label.clone(),
            vbp_study,
            vbp_edge_opens: None,
        }
    }
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
            vbp_study: None,
            vbp_edge_opens: None,
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
            vbp_study: None,
            vbp_edge_opens: None,
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
        crate::style::theme::rgba_to_iced_color(self.style.stroke_color)
    }

    /// Get the fill color as an iced Color (if any)
    pub fn fill_color(&self) -> Option<Color> {
        self.style
            .fill_color
            .map(crate::style::theme::rgba_to_iced_color)
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

    /// Get the handle positions for selection (in screen coordinates).
    ///
    /// For VBP drawings, handles appear on the left/right edges at the
    /// candle open price instead of at the corner anchor points.
    pub fn handle_positions(&self, state: &ViewState, bounds: Size) -> Vec<Point> {
        if self.tool == DrawingTool::VolumeProfile
            && let Some((left_open, right_open)) = self.vbp_edge_opens
            && self.points.len() >= 2
        {
            let t0 = self.points[0].time;
            let t1 = self.points[1].time;
            let left = DrawingPoint::new(left_open, t0.min(t1));
            let right = DrawingPoint::new(right_open, t0.max(t1));
            return vec![
                left.as_screen_point(state, bounds),
                right.as_screen_point(state, bounds),
            ];
        }
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
        let mut style = self.style.clone();
        // Export VBP config from embedded study before serialization
        if let Some(ref study) = self.vbp_study {
            style.vbp_config = Some(data::VbpDrawingConfig {
                params: study.export_config(),
            });
        }
        SerializableDrawing {
            id: self.id,
            tool: self.tool,
            points: self.points.iter().map(|p| p.to_serializable()).collect(),
            style,
            visible: self.visible,
            locked: self.locked,
            label: self.label.clone(),
        }
    }

    /// Create from serializable format
    pub fn from_serializable(drawing: &SerializableDrawing) -> Self {
        let points: Vec<DrawingPoint> =
            drawing.points.iter().map(DrawingPoint::from).collect();
        let confirmed_count = points.len();

        // Reconstruct VBP study for VolumeProfile drawings
        let vbp_study = if drawing.tool == DrawingTool::VolumeProfile
            && points.len() >= 2
        {
            let t1 = points[0].time;
            let t2 = points[1].time;
            let (start, end) = (t1.min(t2), t1.max(t2));
            let mut study = study::orderflow::VbpStudy::for_range(start, end);
            if let Some(ref cfg) = drawing.style.vbp_config {
                study.import_config(&cfg.params);
            }
            // Force Custom period after import to preserve drawing anchors
            study.set_range(start, end);
            Some(Box::new(study))
        } else {
            None
        };

        Self {
            id: drawing.id,
            tool: drawing.tool,
            points,
            confirmed_count,
            style: drawing.style.clone(),
            visible: drawing.visible,
            locked: drawing.locked,
            label: drawing.label.clone(),
            vbp_study,
            vbp_edge_opens: None,
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
