//! Drawing interaction handlers

use super::{DrawingEditMode, DrawingState, Interaction};
use crate::chart::Message;
use crate::chart::core::Chart;
use crate::chart::core::tokens;
use crate::drawing::{DrawingId, DrawingTool};
use iced::{Point, widget::canvas};

/// Handle left click during Drawing state
pub fn handle_drawing_click(
    interaction: &mut Interaction,
    tool: DrawingTool,
    draw_state: DrawingState,
    cursor_in_bounds: Point,
    shift_held: bool,
) -> Option<canvas::Action<Message>> {
    match draw_state {
        DrawingState::Placing => {
            // First click - start the drawing
            *interaction = Interaction::Drawing {
                tool,
                state: DrawingState::Previewing {
                    start: cursor_in_bounds,
                },
            };
            Some(
                canvas::Action::publish(Message::DrawingClick(cursor_in_bounds, shift_held))
                    .and_capture(),
            )
        }
        DrawingState::Previewing { .. } => {
            // Second click - complete the drawing
            // Return to placing state for next drawing
            *interaction = Interaction::Drawing {
                tool,
                state: DrawingState::Placing,
            };
            Some(
                canvas::Action::publish(Message::DrawingClick(cursor_in_bounds, shift_held))
                    .and_capture(),
            )
        }
    }
}

/// Handle cursor movement during drawing mode
///
/// When a pending drawing exists, sends DrawingMove to update the preview.
/// When no pending drawing exists (e.g. after completing a drawing),
/// falls through to CrosshairMoved so the crosshair keeps updating.
pub fn handle_drawing_move<T: Chart>(
    chart: &T,
    cursor_position: Option<Point>,
    shift_held: bool,
) -> Option<canvas::Action<Message>> {
    if let Some(cursor_pos) = cursor_position {
        if chart.has_pending_drawing() {
            Some(canvas::Action::publish(Message::DrawingMove(
                cursor_pos, shift_held,
            )))
        } else {
            // No pending drawing: let the crosshair update normally
            Some(canvas::Action::publish(Message::CrosshairMoved(
                cursor_position,
            )))
        }
    } else {
        Some(canvas::Action::publish(Message::CrosshairMoved(None)))
    }
}

/// Handle left click when entering drawing mode from None/Panning/Zoomin state
pub fn handle_enter_drawing<T: Chart>(
    chart: &T,
    interaction: &mut Interaction,
    cursor_in_bounds: Point,
    shift_held: bool,
) -> Option<canvas::Action<Message>> {
    let active_tool = chart.active_drawing_tool();
    if active_tool != DrawingTool::None {
        *interaction = Interaction::Drawing {
            tool: active_tool,
            state: DrawingState::Previewing {
                start: cursor_in_bounds,
            },
        };
        return Some(
            canvas::Action::publish(Message::DrawingClick(cursor_in_bounds, shift_held))
                .and_capture(),
        );
    }
    None
}

/// Handle left click when no tool is active (selection/editing)
///
/// Priority:
/// 1. Hit test handles first (for already-selected drawings) -> DraggingHandle
/// 2. Hit test drawings -> select drawing (or double-click to open properties)
/// 3. If nothing hit and has selection -> deselect
/// 4. If nothing hit -> return None (caller starts panning)
pub fn handle_selection_click<T: Chart>(
    chart: &T,
    interaction: &mut Interaction,
    cursor_in_bounds: Point,
    last_selection_click: &mut Option<(std::time::Instant, crate::drawing::DrawingId)>,
) -> Option<canvas::Action<Message>> {
    let bounds = chart.state().bounds.size();

    // 1. Check handle hit on already-selected drawings (skip locked)
    if let Some((id, handle_index)) = chart.hit_test_drawing_handle(cursor_in_bounds, bounds)
        && !chart.is_drawing_locked(id)
    {
        *last_selection_click = None;
        *interaction = Interaction::EditingDrawing {
            id,
            edit_mode: DrawingEditMode::DraggingHandle { handle_index },
            last_screen_pos: cursor_in_bounds,
            drag_committed: true, // handle drags are immediate
        };
        return Some(canvas::Action::request_redraw().and_capture());
    }

    // 2. Check drawing body hit
    if let Some(id) = chart.hit_test_drawing(cursor_in_bounds, bounds) {
        // Check for double-click on an already-selected drawing
        if chart.is_drawing_selected(id)
            && let Some((prev_time, prev_id)) = last_selection_click
            && *prev_id == id
            && prev_time.elapsed().as_millis() < tokens::drawing::DOUBLE_CLICK_MS
        {
            *last_selection_click = None;
            return Some(canvas::Action::publish(Message::DrawingDoubleClick(id)).and_capture());
        }

        *last_selection_click = Some((std::time::Instant::now(), id));

        if chart.is_drawing_locked(id) {
            // Locked: select but do not capture; drag will start pan on first move
            *interaction = Interaction::SelectedLockedDrawing {
                press_pos: cursor_in_bounds,
            };
            return Some(canvas::Action::publish(Message::DrawingSelect(id)));
        }

        *interaction = Interaction::EditingDrawing {
            id,
            edit_mode: DrawingEditMode::Moving,
            last_screen_pos: cursor_in_bounds,
            drag_committed: false, // wait for drag threshold
        };
        return Some(canvas::Action::publish(Message::DrawingSelect(id)).and_capture());
    }

    *last_selection_click = None;

    // 3. If we had a selection, deselect
    if chart.has_drawing_selection() {
        return Some(canvas::Action::publish(Message::DrawingDeselect).and_capture());
    }

    // 4. Nothing hit, no selection - let caller handle (pan)
    None
}

/// Handle cursor movement during editing (moving/dragging handle)
pub fn handle_editing_move(
    edit_mode: DrawingEditMode,
    cursor_position: Option<Point>,
    shift_held: bool,
    start_pos: Point,
    drag_committed: &mut bool,
) -> Option<canvas::Action<Message>> {
    if let Some(cursor_pos) = cursor_position {
        // Check drag threshold for uncommitted moves
        if !*drag_committed {
            let dx = cursor_pos.x - start_pos.x;
            let dy = cursor_pos.y - start_pos.y;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance < tokens::drawing::DRAG_THRESHOLD {
                return Some(canvas::Action::request_redraw().and_capture());
            }
            *drag_committed = true;
        }

        match edit_mode {
            DrawingEditMode::Moving => Some(
                canvas::Action::publish(Message::DrawingDrag(cursor_pos, shift_held)).and_capture(),
            ),
            DrawingEditMode::DraggingHandle { handle_index } => Some(
                canvas::Action::publish(Message::DrawingHandleDrag(
                    cursor_pos,
                    handle_index,
                    shift_held,
                ))
                .and_capture(),
            ),
        }
    } else {
        None
    }
}

/// Handle button release during editing
pub fn handle_editing_release(
    interaction: &mut Interaction,
    _id: DrawingId,
) -> Option<canvas::Action<Message>> {
    *interaction = Interaction::None;
    Some(canvas::Action::publish(Message::DrawingDragEnd))
}
