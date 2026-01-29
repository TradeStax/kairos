//! Drawing Manager
//!
//! Manages all drawings on a chart, including creation, selection, and editing.

use super::drawing::Drawing;
use super::point::DrawingPoint;
use crate::chart::ViewState;
use data::{DrawingId, DrawingStyle, DrawingTool, SerializableDrawing};
use iced::{Point, Size};
use std::collections::HashSet;

/// Manages all drawings on a chart
#[derive(Debug, Clone)]
pub struct DrawingManager {
    /// All completed drawings
    drawings: Vec<Drawing>,
    /// Currently selected drawing IDs
    selected: HashSet<DrawingId>,
    /// Drawing currently being created
    pending: Option<Drawing>,
    /// Currently active drawing tool
    active_tool: DrawingTool,
    /// Whether to snap points to candles
    snap_enabled: bool,
    /// Default style for new drawings
    default_style: DrawingStyle,
}

impl Default for DrawingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawingManager {
    /// Create a new drawing manager
    pub fn new() -> Self {
        Self {
            drawings: Vec::new(),
            selected: HashSet::new(),
            pending: None,
            active_tool: DrawingTool::None,
            snap_enabled: true,
            default_style: DrawingStyle::default(),
        }
    }

    /// Create from serialized drawings
    pub fn from_serializable(drawings: Vec<SerializableDrawing>) -> Self {
        Self {
            drawings: drawings.iter().map(Drawing::from).collect(),
            selected: HashSet::new(),
            pending: None,
            active_tool: DrawingTool::None,
            snap_enabled: true,
            default_style: DrawingStyle::default(),
        }
    }

    /// Get the currently active drawing tool
    pub fn active_tool(&self) -> DrawingTool {
        self.active_tool
    }

    /// Set the active drawing tool
    pub fn set_tool(&mut self, tool: DrawingTool) {
        // Cancel any pending drawing when switching tools
        self.pending = None;
        self.active_tool = tool;
    }

    /// Check if snap is enabled
    pub fn snap_enabled(&self) -> bool {
        self.snap_enabled
    }

    /// Toggle snap mode
    pub fn toggle_snap(&mut self) {
        self.snap_enabled = !self.snap_enabled;
    }

    /// Set snap mode
    pub fn set_snap(&mut self, enabled: bool) {
        self.snap_enabled = enabled;
    }

    /// Get the default style
    pub fn default_style(&self) -> &DrawingStyle {
        &self.default_style
    }

    /// Set the default style
    pub fn set_default_style(&mut self, style: DrawingStyle) {
        self.default_style = style;
    }

    /// Start a new drawing at the given point
    pub fn start_drawing(&mut self, point: DrawingPoint) {
        if self.active_tool == DrawingTool::None {
            return;
        }

        let mut drawing = Drawing::with_style(self.active_tool, self.default_style.clone());
        drawing.add_point(point);

        // For single-point drawings, complete immediately
        if drawing.is_complete() {
            self.drawings.push(drawing);
            self.pending = None;
        } else {
            self.pending = Some(drawing);
        }
    }

    /// Update the preview point for the pending drawing
    pub fn update_preview(&mut self, point: DrawingPoint) {
        if let Some(ref mut drawing) = self.pending {
            drawing.update_preview_point(point);
        }
    }

    /// Complete the pending drawing with the final point
    pub fn complete_drawing(&mut self, point: DrawingPoint) -> Option<DrawingId> {
        if let Some(mut drawing) = self.pending.take() {
            drawing.add_point(point);

            if drawing.is_complete() {
                let id = drawing.id;
                self.drawings.push(drawing);
                return Some(id);
            } else {
                // Not complete yet, put it back
                self.pending = Some(drawing);
            }
        }
        None
    }

    /// Cancel the pending drawing
    pub fn cancel_pending(&mut self) {
        self.pending = None;
    }

    /// Get the pending drawing (if any)
    pub fn pending(&self) -> Option<&Drawing> {
        self.pending.as_ref()
    }

    /// Check if there's a pending drawing
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Get all completed drawings
    pub fn drawings(&self) -> &[Drawing] {
        &self.drawings
    }

    /// Get a drawing by ID
    pub fn get(&self, id: DrawingId) -> Option<&Drawing> {
        self.drawings.iter().find(|d| d.id == id)
    }

    /// Get a mutable reference to a drawing by ID
    pub fn get_mut(&mut self, id: DrawingId) -> Option<&mut Drawing> {
        self.drawings.iter_mut().find(|d| d.id == id)
    }

    /// Select a drawing
    pub fn select(&mut self, id: DrawingId) {
        self.selected.clear();
        self.selected.insert(id);
    }

    /// Add a drawing to the selection
    pub fn add_to_selection(&mut self, id: DrawingId) {
        self.selected.insert(id);
    }

    /// Clear the selection
    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    /// Check if a drawing is selected
    pub fn is_selected(&self, id: DrawingId) -> bool {
        self.selected.contains(&id)
    }

    /// Get selected drawing IDs
    pub fn selected_ids(&self) -> &HashSet<DrawingId> {
        &self.selected
    }

    /// Delete selected drawings
    pub fn delete_selected(&mut self) {
        self.drawings.retain(|d| !self.selected.contains(&d.id));
        self.selected.clear();
    }

    /// Delete a specific drawing
    pub fn delete(&mut self, id: DrawingId) {
        self.drawings.retain(|d| d.id != id);
        self.selected.remove(&id);
    }

    /// Hit test all drawings at a screen point, returns the topmost hit
    pub fn hit_test(
        &self,
        screen_point: Point,
        state: &ViewState,
        bounds: Size,
        tolerance: f32,
    ) -> Option<DrawingId> {
        // Test in reverse order (topmost first)
        for drawing in self.drawings.iter().rev() {
            if drawing.hit_test(screen_point, state, bounds, tolerance) {
                return Some(drawing.id);
            }
        }
        None
    }

    /// Hit test handles of selected drawings
    pub fn hit_test_handle(
        &self,
        screen_point: Point,
        state: &ViewState,
        bounds: Size,
        handle_size: f32,
    ) -> Option<(DrawingId, usize)> {
        for &id in &self.selected {
            if let Some(drawing) = self.get(id) {
                if let Some(handle_index) =
                    drawing.hit_test_handle(screen_point, state, bounds, handle_size)
                {
                    return Some((id, handle_index));
                }
            }
        }
        None
    }

    /// Move a drawing point (for handle dragging)
    pub fn move_point(&mut self, id: DrawingId, point_index: usize, new_point: DrawingPoint) {
        if let Some(drawing) = self.get_mut(id) {
            if point_index < drawing.points.len() && !drawing.locked {
                drawing.points[point_index] = new_point;
            }
        }
    }

    /// Move entire drawing by offset
    pub fn move_drawing(&mut self, id: DrawingId, delta_time: i64, delta_price: i64) {
        if let Some(drawing) = self.get_mut(id) {
            if drawing.locked {
                return;
            }
            for point in &mut drawing.points {
                point.time = (point.time as i64 + delta_time).max(0) as u64;
                point.price = exchange::util::Price::from_units(point.price.units + delta_price);
            }
        }
    }

    /// Serialize all drawings for persistence
    pub fn to_serializable(&self) -> Vec<SerializableDrawing> {
        self.drawings.iter().map(|d| d.to_serializable()).collect()
    }

    /// Load drawings from serialized format
    pub fn load_drawings(&mut self, drawings: Vec<SerializableDrawing>) {
        self.drawings = drawings.iter().map(Drawing::from).collect();
        self.selected.clear();
    }

    /// Check if there are any drawings
    pub fn is_empty(&self) -> bool {
        self.drawings.is_empty() && self.pending.is_none()
    }

    /// Get the number of drawings
    pub fn len(&self) -> usize {
        self.drawings.len()
    }
}
