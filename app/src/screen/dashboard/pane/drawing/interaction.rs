use super::super::State;
use crate::chart::ViewState;
use crate::chart::drawing::{ChartDrawingAccess, Drawing, DrawingManager, DrawingPoint, snap};
use crate::drawing::{DrawingId, DrawingTool};
use iced::Size;

/// Get the anchor point in screen coords for shift-constrained creation.
/// Returns `(tool, anchor_screen_point)` if a pending drawing has an anchor.
fn creation_anchor(
    drawings: &DrawingManager,
    state: &ViewState,
    bounds: Size,
) -> Option<(DrawingTool, iced::Point)> {
    let pending = drawings.pending()?;
    if pending.points.is_empty() {
        return None;
    }
    Some((
        pending.tool,
        pending.points[0].as_screen_point(state, bounds),
    ))
}

/// Apply shift constraint for drawing creation, using the pending
/// drawing's first point as anchor.
fn constrain_for_creation(
    chart: &dyn ChartDrawingAccess,
    screen_point: iced::Point,
    shift_held: bool,
) -> iced::Point {
    if !shift_held {
        return screen_point;
    }
    let state = chart.view_state();
    let bounds = state.bounds.size();
    creation_anchor(chart.drawings(), state, bounds)
        .map(|(tool, anchor)| snap::constrain_creation(tool, anchor, screen_point))
        .unwrap_or(screen_point)
}

/// Convert a screen point to a DrawingPoint using the chart's current
/// view state and snap setting.
fn screen_to_drawing_point(
    chart: &dyn ChartDrawingAccess,
    screen_point: iced::Point,
) -> DrawingPoint {
    let state = chart.view_state();
    let bounds = state.bounds.size();
    let snap = chart.drawings().snap_enabled();
    DrawingPoint::from_screen(screen_point, state, bounds, snap)
}

impl State {
    /// Handle drawing click at a screen position.
    /// Returns `true` if a drawing was completed and the tool was
    /// auto-switched to selection mode.
    pub(in crate::screen::dashboard::pane) fn handle_drawing_click(
        &mut self,
        screen_point: iced::Point,
        shift_held: bool,
    ) -> bool {
        let Some(chart) = self.content.drawing_chart_mut() else {
            return false;
        };

        let constrained = constrain_for_creation(chart, screen_point, shift_held);
        let mut point = screen_to_drawing_point(chart, constrained);

        // Snap first VBP point's Y to candle open price
        if chart.drawings().active_tool().is_vbp()
            && !chart.drawings().has_pending()
            && let Some(open) = chart.candle_open_at_time(point.time)
        {
            point.price = open;
        }

        let completed_id = if chart.drawings().has_pending() {
            chart.drawings_mut().complete_drawing(point)
        } else {
            chart.drawings_mut().start_drawing(point)
        };

        if let Some(id) = completed_id {
            // Check if this was an AiContext drawing before resetting tool
            let is_ai_context = chart
                .drawings()
                .get(id)
                .is_some_and(|d| d.tool == DrawingTool::AiContext);

            chart.drawings_mut().set_tool(DrawingTool::None);
            chart.drawings_mut().select(id);
            chart.compute_pending_vbp();
            chart.invalidate_all_drawing_caches();

            if is_ai_context {
                self.show_ai_context_bubble(id);
            }

            return true;
        }
        chart.invalidate_crosshair_cache();
        false
    }

    /// Handle drawing move (update preview)
    pub(in crate::screen::dashboard::pane) fn handle_drawing_move(
        &mut self,
        screen_point: iced::Point,
        shift_held: bool,
    ) {
        let Some(chart) = self.content.drawing_chart_mut() else {
            return;
        };
        if !chart.drawings().has_pending() {
            return;
        }

        let constrained = constrain_for_creation(chart, screen_point, shift_held);
        let mut point = screen_to_drawing_point(chart, constrained);

        // Snap VBP preview Y to candle open price
        if chart.drawings().active_tool().is_vbp()
            && let Some(open) = chart.candle_open_at_time(point.time)
        {
            point.price = open;
        }

        chart.drawings_mut().update_preview(point);
        chart.invalidate_crosshair_cache();
    }

    /// Handle drawing cancel (Escape key)
    pub(in crate::screen::dashboard::pane) fn handle_drawing_cancel(&mut self) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().cancel_pending();
            chart.invalidate_crosshair_cache();
        }
    }

    /// Handle drawing delete (Delete/Backspace key)
    pub(in crate::screen::dashboard::pane) fn handle_drawing_delete(&mut self) {
        self.ai_context_bubble = None;
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().delete_selected();
            chart.invalidate_all_drawing_caches();
        }
    }

    /// Handle drawing selection
    pub(in crate::screen::dashboard::pane) fn handle_drawing_select(&mut self, id: DrawingId) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            let is_ai_context = chart
                .drawings()
                .get(id)
                .is_some_and(|d| d.tool == DrawingTool::AiContext);
            chart.drawings_mut().select(id);
            chart.invalidate_all_drawing_caches();

            if is_ai_context {
                self.show_ai_context_bubble(id);
            } else {
                self.ai_context_bubble = None;
            }
        }
    }

    /// Handle drawing deselection
    pub(in crate::screen::dashboard::pane) fn handle_drawing_deselect(&mut self) {
        self.ai_context_bubble = None;
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().clear_selection();
            chart.invalidate_all_drawing_caches();
        }
    }

    /// Handle whole-drawing drag (moving entire drawing)
    pub(in crate::screen::dashboard::pane) fn handle_drawing_drag(
        &mut self,
        screen_point: iced::Point,
        shift_held: bool,
    ) {
        let Some(chart) = self.content.drawing_chart_mut() else {
            return;
        };

        // Axis-lock when shift held
        let constrained = if shift_held {
            chart
                .drawings()
                .drag_start_screen()
                .map(|start| snap::constrain_axis(start, screen_point))
                .unwrap_or(screen_point)
        } else {
            screen_point
        };

        let point = screen_to_drawing_point(chart, constrained);
        let selected: Vec<DrawingId> = chart.drawings().selected_ids().iter().copied().collect();

        if let Some(&id) = selected.first() {
            if chart.drawings().is_dragging() {
                chart.drawings_mut().update_drag(id, point);
            } else {
                chart.drawings_mut().start_drag(point, id, screen_point);
            }
        }
        chart.invalidate_all_drawing_caches();
    }

    /// Handle single-handle drag
    pub(in crate::screen::dashboard::pane) fn handle_drawing_handle_drag(
        &mut self,
        screen_point: iced::Point,
        handle_index: usize,
        shift_held: bool,
    ) {
        let Some(chart) = self.content.drawing_chart_mut() else {
            return;
        };

        let selected: Vec<DrawingId> = chart.drawings().selected_ids().iter().copied().collect();
        let Some(&id) = selected.first() else { return };

        let is_vbp = chart.drawings().get(id).is_some_and(|d| d.tool.is_vbp());

        // Apply shift constraint based on tool/handle (skip for VBP)
        let constrained = if shift_held && !is_vbp {
            let state = chart.view_state();
            let bounds = state.bounds.size();
            chart
                .drawings()
                .get(id)
                .map(|d| {
                    snap::constrain_handle(
                        d.tool,
                        &d.points,
                        handle_index,
                        state,
                        bounds,
                        screen_point,
                    )
                })
                .unwrap_or(screen_point)
        } else {
            screen_point
        };

        // VBP handles always snap to candle boundaries
        let point = if is_vbp {
            let state = chart.view_state();
            let bounds = state.bounds.size();
            DrawingPoint::from_screen(constrained, state, bounds, true)
        } else {
            screen_to_drawing_point(chart, constrained)
        };

        if !chart.drawings().is_dragging() {
            chart.drawings_mut().start_drag(point, id, screen_point);
        }
        chart
            .drawings_mut()
            .update_handle_drag(id, handle_index, point);
        chart.invalidate_all_drawing_caches();
    }

    /// Handle drawing drag end
    pub(in crate::screen::dashboard::pane) fn handle_drawing_drag_end(&mut self) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().end_drag();
            chart.compute_pending_vbp();
            chart.invalidate_all_drawing_caches();
        }
    }

    /// Get the first selected drawing ID, if any
    pub(in crate::screen::dashboard::pane) fn get_selected_drawing_id(&self) -> Option<DrawingId> {
        self.content
            .drawing_chart()
            .and_then(|c| c.drawings().selected_ids().iter().next().copied())
    }

    /// Get the locked state for a drawing by ID
    pub(in crate::screen::dashboard::pane) fn get_drawing_locked(&self, id: DrawingId) -> bool {
        self.content
            .drawing_chart()
            .and_then(|c| c.drawings().get(id))
            .is_some_and(|d| d.locked)
    }

    /// Start clone placement mode for a drawing
    pub(in crate::screen::dashboard::pane) fn handle_drawing_clone(&mut self, id: DrawingId) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            if let Some(cloned_drawing) = chart.drawings().get(id).map(|original| {
                let mut cloned = Drawing::from_serializable(&original.to_serializable());
                cloned.id = DrawingId::new();
                cloned
            }) {
                chart.drawings_mut().start_clone_placement(cloned_drawing);
            }
            chart.invalidate_crosshair_cache();
        }
    }

    /// Update clone placement position as cursor moves
    pub(in crate::screen::dashboard::pane) fn handle_clone_move(
        &mut self,
        screen_point: iced::Point,
    ) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            let point = screen_to_drawing_point(chart, screen_point);
            chart.drawings_mut().update_clone_position(point);
            chart.invalidate_crosshair_cache();
        }
    }

    /// Confirm clone placement at cursor position
    pub(in crate::screen::dashboard::pane) fn handle_clone_confirm(
        &mut self,
        screen_point: iced::Point,
    ) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            let point = screen_to_drawing_point(chart, screen_point);
            chart.drawings_mut().update_clone_position(point);
            if let Some(id) = chart.drawings_mut().confirm_clone_placement() {
                chart.drawings_mut().select(id);
            }
            chart.invalidate_all_drawing_caches();
        }
    }

    /// Cancel clone placement
    pub(in crate::screen::dashboard::pane) fn handle_clone_cancel(&mut self) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().cancel_clone_placement();
            chart.invalidate_crosshair_cache();
        }
    }
}
