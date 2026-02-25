//! Drawing Manager
//!
//! Manages all drawings on a chart, including creation, selection, and editing.

use super::drawing::Drawing;
use super::point::DrawingPoint;
use crate::chart::ViewState;
use data::{
    DrawingId, DrawingStyle, DrawingTool, LineStyle, PositionCalcConfig, SerializableDrawing,
    VbpDrawingConfig,
};
use exchange::util::Price as ExchangePrice;
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
    /// VBP drawings that need (re)computation
    vbp_needs_compute: Vec<DrawingId>,
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
            vbp_needs_compute: Vec::new(),
        }
    }

    /// Create from serialized drawings
    pub fn from_serializable(drawings: Vec<SerializableDrawing>) -> Self {
        let drawings: Vec<Drawing> = drawings.iter().map(Drawing::from).collect();
        // Collect VBP drawings that need initial computation
        let vbp_needs_compute: Vec<DrawingId> = drawings
            .iter()
            .filter(|d| d.tool == DrawingTool::VolumeProfile)
            .map(|d| d.id)
            .collect();
        Self {
            drawings,
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
            vbp_needs_compute,
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

    /// Get the default style for a tool, using tool-specific overrides
    /// for calculator tools. Colors match the theme palette
    /// (success = green/target, danger = red/stop).
    fn style_for_tool(&self, tool: DrawingTool) -> DrawingStyle {
        match tool {
            DrawingTool::BuyCalculator => DrawingStyle {
                stroke_color: PositionCalcConfig::DEFAULT_TARGET_COLOR,
                stroke_width: 1.5,
                line_style: LineStyle::Dashed,
                position_calc: Some(PositionCalcConfig::default()),
                ..self.default_style.clone()
            },
            DrawingTool::SellCalculator => DrawingStyle {
                stroke_color: PositionCalcConfig::DEFAULT_STOP_COLOR,
                stroke_width: 1.5,
                line_style: LineStyle::Dashed,
                position_calc: Some(PositionCalcConfig::default()),
                ..self.default_style.clone()
            },
            DrawingTool::VolumeProfile => DrawingStyle {
                stroke_color: data::SerializableColor::new(0.95, 0.55, 0.15, 0.8),
                stroke_width: 1.0,
                line_style: LineStyle::Solid,
                fill_color: Some(data::SerializableColor::new(
                    0.95, 0.55, 0.15, 0.15,
                )),
                fill_opacity: 0.15,
                vbp_config: Some(VbpDrawingConfig::default()),
                ..self.default_style.clone()
            },
            DrawingTool::AiContext => DrawingStyle {
                stroke_color: data::SerializableColor::new(0.35, 0.55, 0.95, 0.7),
                stroke_width: 1.0,
                line_style: LineStyle::Dashed,
                fill_color: Some(data::SerializableColor::new(
                    0.35, 0.55, 0.95, 0.08,
                )),
                fill_opacity: 0.08,
                ..self.default_style.clone()
            },
            _ => self.default_style.clone(),
        }
    }

    /// Auto-generate the 3rd point (stop) for calculator tools at 1:1 R:R.
    fn on_drawing_completed(drawing: &mut Drawing) {
        match drawing.tool {
            DrawingTool::BuyCalculator | DrawingTool::SellCalculator => {
                if drawing.points.len() >= 2 {
                    let entry_price = drawing.points[0].price.units();
                    let target_price = drawing.points[1].price.units();
                    let delta = target_price - entry_price;
                    let stop_price = ExchangePrice::from_units(entry_price - delta);
                    drawing.points.push(DrawingPoint::new(
                        stop_price,
                        drawing.points[1].time,
                    ));
                }
            }
            DrawingTool::VolumeProfile => {
                if drawing.points.len() >= 2 {
                    let t1 = drawing.points[0].time;
                    let t2 = drawing.points[1].time;
                    let (start, end) = (t1.min(t2), t1.max(t2));
                    let mut study =
                        study::orderflow::VbpStudy::for_range(start, end);
                    if let Some(ref cfg) = drawing.style.vbp_config {
                        study.import_config(&cfg.params);
                    }
                    // Force Custom period after import (import may
                    // have overridden with a different period value)
                    study.set_range(start, end);
                    drawing.vbp_study = Some(Box::new(study));
                }
            }
            DrawingTool::AiContext => {} // Handled by pane drawing logic
            _ => {}
        }
    }

    /// Start a new drawing at the given point.
    /// Returns `Some(id)` if the drawing completed immediately (single-point tools).
    pub fn start_drawing(&mut self, point: DrawingPoint) -> Option<DrawingId> {
        if self.active_tool == DrawingTool::None {
            return None;
        }

        let style = self.style_for_tool(self.active_tool);
        let mut drawing = Drawing::with_style(self.active_tool, style);
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
                Self::on_drawing_completed(&mut drawing);
                let id = drawing.id;
                let is_vbp = drawing.tool == DrawingTool::VolumeProfile;
                self.push_undo(DrawingOp::Add(drawing.to_serializable()));
                self.drawings.push(drawing);
                if is_vbp {
                    self.queue_vbp_compute(id);
                }
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
            Self::enforce_calculator_constraints(drawing, point_index);
        }
    }

    /// Keep calculator stop time synced with target time.
    fn enforce_calculator_constraints(drawing: &mut Drawing, changed_index: usize) {
        match drawing.tool {
            DrawingTool::BuyCalculator | DrawingTool::SellCalculator => {
                if drawing.points.len() >= 3 {
                    match changed_index {
                        1 => {
                            // Target moved — sync stop time
                            drawing.points[2].time = drawing.points[1].time;
                        }
                        2 => {
                            // Stop moved — lock X to target's X
                            drawing.points[2].time = drawing.points[1].time;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    /// Move entire drawing by offset.
    ///
    /// VBP drawings only move horizontally — Y bounds are auto-fitted
    /// on drag end.
    pub fn move_drawing(&mut self, id: DrawingId, delta_time: i64, delta_price: i64) {
        if let Some(drawing) = self.get_mut(id) {
            if drawing.locked {
                return;
            }
            let skip_price = drawing.tool == DrawingTool::VolumeProfile;
            for point in &mut drawing.points {
                point.time = (point.time as i64 + delta_time).max(0) as u64;
                if !skip_price {
                    point.price =
                        exchange::util::Price::from_units(point.price.units() + delta_price);
                }
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
            let dp = new_point.price.units() - last.price.units();
            self.move_drawing(id, dt, dp);
            self.last_drag_point = Some(new_point);
        }
    }

    /// Update a single-handle drag.
    ///
    /// VBP drawings only update the time coordinate of the dragged
    /// handle — Y bounds are auto-fitted on drag end.
    pub fn update_handle_drag(
        &mut self,
        id: DrawingId,
        handle_index: usize,
        new_point: DrawingPoint,
    ) {
        if self
            .get(id)
            .is_some_and(|d| d.tool == DrawingTool::VolumeProfile)
        {
            if let Some(drawing) = self.get_mut(id)
                && handle_index < drawing.points.len()
                && !drawing.locked
            {
                drawing.points[handle_index].time = new_point.time;
            }
            self.last_drag_point = Some(new_point);
            return;
        }
        self.move_point(id, handle_index, new_point);
        self.last_drag_point = Some(new_point);
    }

    /// End a drag operation
    pub fn end_drag(&mut self) {
        // Check if we were dragging a VBP drawing that needs recompute
        if let Some((id, _)) = &self.move_edit_before {
            let id = *id;
            if self.get(id).is_some_and(|d| {
                d.tool == DrawingTool::VolumeProfile
            }) {
                self.queue_vbp_compute(id);
            }
        }
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
        // Queue VBP drawings for initial computation
        self.vbp_needs_compute = self
            .drawings
            .iter()
            .filter(|d| d.tool == DrawingTool::VolumeProfile)
            .map(|d| d.id)
            .collect();
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
                    self.drawings.retain(|d| d.id != drawing.id);
                    self.selected.remove(&drawing.id);
                    self.redo_stack.push_back(op);
                }
                DrawingOp::Remove(drawing) => {
                    let id = drawing.id;
                    let is_vbp = drawing.tool == DrawingTool::VolumeProfile;
                    self.drawings.push(Drawing::from(&drawing));
                    if is_vbp {
                        self.queue_vbp_compute(id);
                    }
                    self.redo_stack.push_back(op);
                }
                DrawingOp::Modify { id, before, .. } => {
                    let is_vbp = before.tool == DrawingTool::VolumeProfile;
                    if let Some(pos) =
                        self.drawings.iter().position(|d| d.id == id)
                    {
                        self.drawings[pos] = Drawing::from(&before);
                    }
                    if is_vbp {
                        self.queue_vbp_compute(id);
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
                    let id = drawing.id;
                    let is_vbp = drawing.tool == DrawingTool::VolumeProfile;
                    self.drawings.push(Drawing::from(&drawing));
                    if is_vbp {
                        self.queue_vbp_compute(id);
                    }
                    self.undo_stack.push_back(op);
                }
                DrawingOp::Remove(drawing) => {
                    self.drawings.retain(|d| d.id != drawing.id);
                    self.selected.remove(&drawing.id);
                    self.undo_stack.push_back(op);
                }
                DrawingOp::Modify { id, after, .. } => {
                    let is_vbp = after.tool == DrawingTool::VolumeProfile;
                    if let Some(pos) =
                        self.drawings.iter().position(|d| d.id == id)
                    {
                        self.drawings[pos] = Drawing::from(&after);
                    }
                    if is_vbp {
                        self.queue_vbp_compute(id);
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

    // ── VBP auto-fit ──────────────────────────────────────────────────

    /// Adjust a VBP drawing's Y bounds to match the high/low of candles
    /// within the selected time range.
    pub fn fit_vbp_to_candle_range(
        &mut self,
        id: DrawingId,
        candles: &[data::Candle],
    ) {
        let Some(d) = self.get_mut(id) else { return };
        if d.tool != DrawingTool::VolumeProfile || d.points.len() < 2 {
            return;
        }
        let t1 = d.points[0].time;
        let t2 = d.points[1].time;
        let (start, end) = (t1.min(t2), t1.max(t2));

        let mut range_high = i64::MIN;
        let mut range_low = i64::MAX;
        for c in candles {
            let ct = c.time.0;
            if ct >= start && ct <= end {
                range_high = range_high.max(c.high.units());
                range_low = range_low.min(c.low.units());
            }
        }
        if range_high > range_low {
            d.points[0].price = ExchangePrice::from_units(range_high);
            d.points[1].price = ExchangePrice::from_units(range_low);
        }

        // Cache the open prices at edge candles for handle positioning
        let find_open_at = |time: u64| -> Option<ExchangePrice> {
            let idx = candles.partition_point(|c| c.time.0 < time);
            candles
                .get(idx)
                .or_else(|| idx.checked_sub(1).and_then(|i| candles.get(i)))
                .map(|c| ExchangePrice::from_units(c.open.units()))
        };
        if let (Some(lo), Some(ro)) = (find_open_at(start), find_open_at(end))
        {
            d.vbp_edge_opens = Some((lo, ro));
        }
    }

    // ── VBP computation queue ──────────────────────────────────────────

    /// Drain pending VBP computation requests.
    pub fn drain_vbp_computations(&mut self) -> Vec<DrawingId> {
        std::mem::take(&mut self.vbp_needs_compute)
    }

    /// Queue a VBP drawing for recomputation.
    pub fn queue_vbp_compute(&mut self, id: DrawingId) {
        if !self.vbp_needs_compute.contains(&id) {
            self.vbp_needs_compute.push(id);
        }
    }

    // ── Clone placement ────────────────────────────────────────────────

    /// Start clone placement mode: the drawing follows the cursor until
    /// the user clicks to confirm.
    pub fn start_clone_placement(&mut self, drawing: Drawing) {
        let original_points = drawing.points.clone();
        let n = original_points.len().max(1) as i64;
        let center_time = original_points.iter().map(|p| p.time as i64).sum::<i64>() / n;
        let center_price = original_points.iter().map(|p| p.price.units()).sum::<i64>() / n;

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
            let dp = cursor.price.units() - placement.center_price;
            for (i, point) in placement.drawing.points.iter_mut().enumerate() {
                point.time = (placement.original_points[i].time as i64 + dt).max(0) as u64;
                point.price = exchange::util::Price::from_units(
                    placement.original_points[i].price.units() + dp,
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
