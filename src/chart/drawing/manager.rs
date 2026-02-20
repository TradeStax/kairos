//! Drawing Manager
//!
//! Manages all drawings on a chart, including creation, selection, and editing.

use super::drawing::Drawing;
use super::point::DrawingPoint;
use crate::chart::ViewState;
use data::{DrawingId, DrawingStyle, DrawingTool, SerializableDrawing};
use iced::{Point, Size};
use std::collections::{HashSet, VecDeque};

/// Maximum number of undo operations to keep
const MAX_UNDO_STACK: usize = 50;

/// An undoable drawing operation
#[derive(Debug, Clone)]
enum DrawingOp {
    Add(SerializableDrawing),
    Remove(SerializableDrawing),
    Modify {
        id: DrawingId,
        before: SerializableDrawing,
        after: SerializableDrawing,
    },
}

/// State for a clone being placed by the user
#[derive(Debug, Clone)]
struct ClonePlacement {
    /// The cloned drawing being placed
    drawing: Drawing,
    /// Original points (before any movement)
    original_points: Vec<DrawingPoint>,
    /// Center of original points (used as cursor anchor)
    center_time: i64,
    center_price: i64,
}

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
    /// Undo stack
    undo_stack: VecDeque<DrawingOp>,
    /// Redo stack
    redo_stack: VecDeque<DrawingOp>,
    /// Saved state for in-progress move/edit operations
    move_edit_before: Option<(DrawingId, SerializableDrawing)>,
    /// Last cursor position during drag (for relative movement)
    last_drag_point: Option<DrawingPoint>,
    /// Screen position where the current drag started (for axis-lock constraint)
    drag_start_screen: Option<Point>,
    /// Clone placement in progress (user is positioning a cloned drawing)
    clone_placement: Option<ClonePlacement>,
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
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            move_edit_before: None,
            last_drag_point: None,
            drag_start_screen: None,
            clone_placement: None,
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
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            move_edit_before: None,
            last_drag_point: None,
            drag_start_screen: None,
            clone_placement: None,
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

    /// Start a new drawing at the given point.
    /// Returns `Some(id)` if the drawing completed immediately (single-point tools).
    pub fn start_drawing(&mut self, point: DrawingPoint) -> Option<DrawingId> {
        if self.active_tool == DrawingTool::None {
            return None;
        }

        let mut drawing = Drawing::with_style(self.active_tool, self.default_style.clone());
        drawing.add_point(point);

        // For single-point drawings, complete immediately
        if drawing.is_complete() {
            let id = drawing.id;
            self.push_undo(DrawingOp::Add(drawing.to_serializable()));
            self.drawings.push(drawing);
            self.pending = None;
            Some(id)
        } else {
            self.pending = Some(drawing);
            None
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
                self.push_undo(DrawingOp::Add(drawing.to_serializable()));
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
        for drawing in &self.drawings {
            if self.selected.contains(&drawing.id) {
                self.undo_stack
                    .push_back(DrawingOp::Remove(drawing.to_serializable()));
            }
        }
        self.redo_stack.clear();
        self.trim_undo_stack();
        self.drawings.retain(|d| !self.selected.contains(&d.id));
        self.selected.clear();
    }

    /// Add a completed drawing directly (e.g. for cloning)
    pub fn add_drawing(&mut self, drawing: Drawing) {
        self.push_undo(DrawingOp::Add(drawing.to_serializable()));
        self.drawings.push(drawing);
    }

    /// Delete a specific drawing
    pub fn delete(&mut self, id: DrawingId) {
        if let Some(drawing) = self.get(id) {
            self.push_undo(DrawingOp::Remove(drawing.to_serializable()));
        }
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
            if let Some(drawing) = self.get(id)
                && let Some(handle_index) =
                    drawing.hit_test_handle(screen_point, state, bounds, handle_size)
            {
                return Some((id, handle_index));
            }
        }
        None
    }

    /// Move a drawing point (for handle dragging)
    pub fn move_point(&mut self, id: DrawingId, point_index: usize, new_point: DrawingPoint) {
        if let Some(drawing) = self.get_mut(id)
            && point_index < drawing.points.len()
            && !drawing.locked
        {
            drawing.points[point_index] = new_point;
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

    /// Start tracking a move/edit operation for undo support.
    ///
    /// Call this before beginning a drag operation. Saves the drawing's
    /// current state so it can be restored on undo.
    pub fn start_move_edit(&mut self, id: DrawingId) {
        // Only save once per drag operation
        if self.move_edit_before.is_some() {
            return;
        }
        if let Some(drawing) = self.get(id) {
            self.move_edit_before = Some((id, drawing.to_serializable()));
        }
    }

    /// Finish tracking a move/edit operation.
    ///
    /// Call this after a drag operation completes (mouse release).
    /// Pushes the Modify operation onto the undo stack.
    pub fn finish_move_edit(&mut self) {
        if let Some((id, before)) = self.move_edit_before.take() {
            if let Some(drawing) = self.get(id) {
                let after = drawing.to_serializable();
                self.push_undo(DrawingOp::Modify { id, before, after });
            }
        }
    }

    /// Record a property change for undo, bypassing the move_edit guard.
    pub fn record_property_change(&mut self, id: DrawingId, before: SerializableDrawing) {
        if let Some(drawing) = self.get(id) {
            let after = drawing.to_serializable();
            self.push_undo(DrawingOp::Modify { id, before, after });
        }
    }

    /// Start a drag operation with relative tracking.
    /// `screen_pos` is the raw screen position for axis-lock reference.
    pub fn start_drag(&mut self, point: DrawingPoint, id: DrawingId, screen_pos: Point) {
        self.start_move_edit(id);
        self.last_drag_point = Some(point);
        self.drag_start_screen = Some(screen_pos);
    }

    /// Get the screen position where the current drag started
    pub fn drag_start_screen(&self) -> Option<Point> {
        self.drag_start_screen
    }

    /// Update a whole-drawing drag with relative movement
    pub fn update_drag(&mut self, id: DrawingId, new_point: DrawingPoint) {
        if let Some(last) = self.last_drag_point {
            let dt = new_point.time as i64 - last.time as i64;
            let dp = new_point.price.units - last.price.units;
            self.move_drawing(id, dt, dp);
            self.last_drag_point = Some(new_point);
        }
    }

    /// Update a single-handle drag
    pub fn update_handle_drag(
        &mut self,
        id: DrawingId,
        handle_index: usize,
        new_point: DrawingPoint,
    ) {
        self.move_point(id, handle_index, new_point);
        self.last_drag_point = Some(new_point);
    }

    /// End a drag operation
    pub fn end_drag(&mut self) {
        self.last_drag_point = None;
        self.drag_start_screen = None;
        self.finish_move_edit();
    }

    /// Check if a drag is in progress
    pub fn is_dragging(&self) -> bool {
        self.last_drag_point.is_some()
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

    /// Undo the last drawing operation. Returns true if an operation was undone.
    pub fn undo(&mut self) -> bool {
        if let Some(op) = self.undo_stack.pop_back() {
            match op.clone() {
                DrawingOp::Add(drawing) => {
                    // Undo an add = remove the drawing
                    self.drawings.retain(|d| d.id != drawing.id);
                    self.selected.remove(&drawing.id);
                    self.redo_stack.push_back(op);
                }
                DrawingOp::Remove(drawing) => {
                    // Undo a remove = re-add the drawing
                    self.drawings.push(Drawing::from(&drawing));
                    self.redo_stack.push_back(op);
                }
                DrawingOp::Modify { id, before, .. } => {
                    // Undo a modify = restore the before state
                    if let Some(pos) = self.drawings.iter().position(|d| d.id == id) {
                        self.drawings[pos] = Drawing::from(&before);
                    }
                    self.redo_stack.push_back(op);
                }
            }
            true
        } else {
            false
        }
    }

    /// Redo the last undone operation. Returns true if an operation was redone.
    pub fn redo(&mut self) -> bool {
        if let Some(op) = self.redo_stack.pop_back() {
            match op.clone() {
                DrawingOp::Add(drawing) => {
                    // Redo an add = add the drawing back
                    self.drawings.push(Drawing::from(&drawing));
                    self.undo_stack.push_back(op);
                }
                DrawingOp::Remove(drawing) => {
                    // Redo a remove = remove the drawing again
                    self.drawings.retain(|d| d.id != drawing.id);
                    self.selected.remove(&drawing.id);
                    self.undo_stack.push_back(op);
                }
                DrawingOp::Modify { id, after, .. } => {
                    // Redo a modify = apply the after state
                    if let Some(pos) = self.drawings.iter().position(|d| d.id == id) {
                        self.drawings[pos] = Drawing::from(&after);
                    }
                    self.undo_stack.push_back(op);
                }
            }
            true
        } else {
            false
        }
    }

    /// Push an operation onto the undo stack, clearing the redo stack
    fn push_undo(&mut self, op: DrawingOp) {
        self.undo_stack.push_back(op);
        self.redo_stack.clear();
        self.trim_undo_stack();
    }

    /// Trim the undo stack to MAX_UNDO_STACK (O(1) per pop from front)
    fn trim_undo_stack(&mut self) {
        while self.undo_stack.len() > MAX_UNDO_STACK {
            self.undo_stack.pop_front();
        }
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    // ── Clone placement ────────────────────────────────────────────────

    /// Start clone placement mode: the drawing follows the cursor until
    /// the user clicks to confirm.
    pub fn start_clone_placement(&mut self, drawing: Drawing) {
        let original_points = drawing.points.clone();
        let n = original_points.len().max(1) as i64;
        let center_time = original_points.iter().map(|p| p.time as i64).sum::<i64>() / n;
        let center_price = original_points.iter().map(|p| p.price.units).sum::<i64>() / n;

        self.clone_placement = Some(ClonePlacement {
            drawing,
            original_points,
            center_time,
            center_price,
        });
    }

    /// Update the clone position so it follows the cursor.
    pub fn update_clone_position(&mut self, cursor: DrawingPoint) {
        if let Some(ref mut placement) = self.clone_placement {
            let dt = cursor.time as i64 - placement.center_time;
            let dp = cursor.price.units - placement.center_price;
            for (i, point) in placement.drawing.points.iter_mut().enumerate() {
                point.time = (placement.original_points[i].time as i64 + dt).max(0) as u64;
                point.price = exchange::util::Price::from_units(
                    placement.original_points[i].price.units + dp,
                );
            }
        }
    }

    /// Confirm clone placement, adding the drawing to the chart.
    /// Returns the new drawing's ID.
    pub fn confirm_clone_placement(&mut self) -> Option<DrawingId> {
        if let Some(placement) = self.clone_placement.take() {
            let id = placement.drawing.id;
            self.push_undo(DrawingOp::Add(placement.drawing.to_serializable()));
            self.drawings.push(placement.drawing);
            Some(id)
        } else {
            None
        }
    }

    /// Cancel clone placement without adding the drawing.
    pub fn cancel_clone_placement(&mut self) {
        self.clone_placement = None;
    }

    /// Check if a clone is being placed.
    pub fn has_clone_pending(&self) -> bool {
        self.clone_placement.is_some()
    }

    /// Get the clone being placed (for rendering).
    pub fn clone_preview(&self) -> Option<&Drawing> {
        self.clone_placement.as_ref().map(|p| &p.drawing)
    }
}
