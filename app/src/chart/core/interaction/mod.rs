//! User Interaction State
//!
//! Handles mouse and keyboard interactions with the chart canvas.

mod drawing;
mod pan_zoom;
mod ruler;

use super::Chart;
use crate::chart::Message;
use crate::components::layout::multi_split::DRAG_SIZE;
use crate::drawing::{DrawingId, DrawingTool};
use crate::style::animation;
use iced::{Point, Rectangle, Vector, keyboard, mouse, widget::canvas};

/// Canvas program state for main chart types (KlineChart, HeatmapChart).
///
/// Wraps `Interaction` plus double-click tracking for drawing properties.
#[derive(Default, Debug, Clone)]
pub struct ChartState {
    pub interaction: Interaction,
    pub last_selection_click: Option<(std::time::Instant, DrawingId)>,
    /// Whether Shift key is currently held (for drawing snap constraints)
    pub shift_held: bool,
    /// Previous cursor position during panning (for velocity calculation)
    pub prev_pan_cursor: Option<Point>,
    /// Smoothed pan velocity (exponential moving average)
    pub pan_velocity: Vector,
    /// Currently selected study overlay index (into chart.studies())
    pub selected_study_overlay: Option<usize>,
    /// Last study overlay click for double-click detection
    pub last_study_overlay_click: Option<(std::time::Instant, usize)>,
}

/// Current interaction mode for the chart
#[derive(Default, Debug, Clone, Copy)]
pub enum Interaction {
    /// No active interaction
    #[default]
    None,
    /// Zooming via drag
    Zoomin {
        /// Last cursor position during zoom
        last_position: Point,
    },
    /// Panning the view
    Panning {
        /// Translation at start of pan
        translation: Vector,
        /// Cursor position at start of pan
        start: Point,
    },
    /// Ruler measurement mode
    Ruler {
        /// Start point of ruler (None if not placed yet)
        start: Option<Point>,
    },
    /// Drawing mode - placing or previewing a drawing
    Drawing {
        /// The tool being used
        tool: DrawingTool,
        /// Current state of the drawing operation
        state: DrawingState,
    },
    /// Editing an existing drawing (moving or dragging handle)
    EditingDrawing {
        /// The drawing being edited
        id: DrawingId,
        /// The edit operation in progress
        edit_mode: DrawingEditMode,
        /// Last screen position (for computing deltas)
        last_screen_pos: Point,
        /// Whether drag threshold has been exceeded
        drag_committed: bool,
    },
    /// Placing a cloned drawing (follows cursor, click to confirm)
    PlacingClone,
    /// Selected a locked drawing; next move starts pan, release clears (no capture)
    SelectedLockedDrawing {
        /// Cursor position at press (used as pan start when user drags)
        press_pos: Point,
    },
    /// Decelerating after a pan gesture ended with velocity
    Decelerating {
        /// Current velocity (screen-space pixels per tick)
        velocity: Vector,
    },
}

/// State of an in-progress drawing operation
#[derive(Debug, Clone, Copy)]
pub enum DrawingState {
    /// Waiting for first click to place initial point
    Placing,
    /// First point placed, showing preview line to cursor
    Previewing {
        /// Screen position of first click
        start: Point,
    },
}

/// Mode for editing an existing drawing
#[derive(Debug, Clone, Copy)]
pub enum DrawingEditMode {
    /// Moving the entire drawing
    Moving,
    /// Dragging a specific handle/point
    DraggingHandle {
        /// Index of the handle being dragged
        handle_index: usize,
    },
}

impl Interaction {
    /// Check if we're in drawing mode
    pub fn is_drawing(&self) -> bool {
        matches!(self, Interaction::Drawing { .. })
    }

    /// Check if we're editing a drawing
    pub fn is_editing_drawing(&self) -> bool {
        matches!(self, Interaction::EditingDrawing { .. })
    }

    /// Get the active drawing tool if in drawing mode
    pub fn drawing_tool(&self) -> Option<DrawingTool> {
        match self {
            Interaction::Drawing { tool, .. } => Some(*tool),
            _ => None,
        }
    }

    /// Enter drawing mode with the given tool
    pub fn enter_drawing_mode(tool: DrawingTool) -> Self {
        if tool == DrawingTool::None {
            Interaction::None
        } else {
            Interaction::Drawing {
                tool,
                state: DrawingState::Placing,
            }
        }
    }
}

/// Resolve `mouse::Interaction` for the "active" interaction modes
/// (Panning, Zoomin, Drawing, EditingDrawing). Returns `None` when
/// the interaction is idle and chart-specific logic should take over.
pub fn base_mouse_interaction(
    interaction: &Interaction,
    bounds: Rectangle,
    cursor: mouse::Cursor,
) -> Option<mouse::Interaction> {
    match interaction {
        Interaction::Panning { .. } => Some(mouse::Interaction::Grabbing),
        Interaction::Zoomin { .. } => Some(mouse::Interaction::ZoomIn),
        Interaction::Drawing { .. } | Interaction::PlacingClone => {
            Some(if cursor.is_over(bounds) {
                mouse::Interaction::Crosshair
            } else {
                mouse::Interaction::default()
            })
        }
        Interaction::EditingDrawing { .. } => Some(if cursor.is_over(bounds) {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::default()
        }),
        _ => None,
    }
}

/// Process canvas interaction events and produce chart messages
///
/// Handles mouse events (pan, zoom, click) and keyboard events (ruler mode, escape).
/// Returns appropriate canvas action with message if interaction occurred.
pub fn canvas_interaction<T: Chart>(
    chart: &T,
    chart_state: &mut ChartState,
    event: &canvas::Event,
    bounds: Rectangle,
    cursor: mouse::Cursor,
) -> Option<canvas::Action<Message>> {
    let interaction = &mut chart_state.interaction;

    // Sync interaction with tool state: if user deactivated the tool via UI,
    // reset the canvas interaction so we can reach selection/panning code paths.
    if let Interaction::Drawing { .. } = interaction
        && chart.active_drawing_tool() == DrawingTool::None
    {
        *interaction = Interaction::None;
    }

    // Sync clone placement state: enter/exit PlacingClone based on DrawingManager
    if chart.has_clone_pending() {
        if !matches!(interaction, Interaction::PlacingClone) {
            *interaction = Interaction::PlacingClone;
        }
    } else if matches!(interaction, Interaction::PlacingClone) {
        *interaction = Interaction::None;
    }

    // Check for bounds change
    if chart.state().bounds != bounds {
        return Some(canvas::Action::publish(Message::BoundsChanged(bounds)));
    }

    let shrunken_bounds = bounds.shrink(DRAG_SIZE * 4.0);
    let cursor_position = cursor.position_in(shrunken_bounds);
    // Canvas-relative cursor matches the frame coordinate space used by drawing to_screen()
    let canvas_cursor = cursor.position_in(bounds);

    // Handle button release - end active interactions
    if let canvas::Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
        match *interaction {
            Interaction::Panning { .. } => {
                let velocity = chart_state.pan_velocity;
                chart_state.prev_pan_cursor = None;
                chart_state.pan_velocity = Vector::ZERO;

                // Transition to deceleration if velocity is significant
                if velocity.x.abs() > 0.5 || velocity.y.abs() > 0.5 {
                    *interaction = Interaction::Decelerating { velocity };
                } else {
                    *interaction = Interaction::None;
                }
            }
            Interaction::Zoomin { .. } => {
                *interaction = Interaction::None;
            }
            Interaction::EditingDrawing { id, .. } => {
                return drawing::handle_editing_release(interaction, id);
            }
            Interaction::SelectedLockedDrawing { .. } => {
                *interaction = Interaction::None;
            }
            _ => {}
        }
    }

    // Stop deceleration on any mouse click or scroll
    if matches!(
        event,
        canvas::Event::Mouse(mouse::Event::ButtonPressed(_))
            | canvas::Event::Mouse(mouse::Event::WheelScrolled { .. })
    ) && matches!(chart_state.interaction, Interaction::Decelerating { .. })
    {
        chart_state.interaction = Interaction::None;
    }

    // Drive deceleration physics on any incoming event while decelerating.
    // Only horizontal momentum is applied (vertical momentum feels disorienting
    // on trading charts where price levels have meaning).
    if let Interaction::Decelerating { ref mut velocity } = chart_state.interaction {
        let state = chart.state();
        let new_x = state.translation.x + velocity.x / state.scaling;
        let new_translation = Vector::new(new_x, state.translation.y);

        velocity.x *= animation::deceleration::FRICTION;
        velocity.y = 0.0;

        if velocity.x.abs() < animation::deceleration::STOP_THRESHOLD {
            chart_state.interaction = Interaction::None;
        }

        return Some(canvas::Action::publish(Message::Translated(
            new_translation,
        )));
    }

    let interaction = &mut chart_state.interaction;

    // Cancel ruler if cursor leaves bounds
    if let Interaction::Ruler { .. } = interaction
        && cursor_position.is_none()
    {
        *interaction = Interaction::None;
    }

    match event {
        canvas::Event::Mouse(mouse_event) => {
            let state = chart.state();

            match mouse_event {
                mouse::Event::ButtonPressed(button) => {
                    // Gate: cursor must be in interactive area
                    let cursor_in_bounds = cursor_position?;
                    // Use canvas-relative cursor for drawing operations
                    let canvas_pos = canvas_cursor.unwrap_or(cursor_in_bounds);

                    if let mouse::Button::Right = button {
                        // Don't open context menu during active
                        // drawing/editing/clone placement
                        if interaction.is_drawing()
                            || interaction.is_editing_drawing()
                            || matches!(interaction, Interaction::PlacingClone)
                        {
                            return Some(canvas::Action::request_redraw().and_capture());
                        }

                        // Hit test study overlay first
                        if let Some(idx) = chart.hit_test_study_overlay(canvas_pos) {
                            return Some(
                                canvas::Action::publish(Message::StudyOverlayContextMenu(
                                    canvas_pos, idx,
                                ))
                                .and_capture(),
                            );
                        }

                        // Hit test to check if right-click is on a drawing
                        let drawing_id = chart.hit_test_drawing(canvas_pos, bounds.size());

                        return Some(
                            canvas::Action::publish(Message::ContextMenu(canvas_pos, drawing_id))
                                .and_capture(),
                        );
                    }

                    if let mouse::Button::Left = button {
                        // Copy values from the match to avoid borrow conflicts
                        let current = *interaction;
                        match current {
                            Interaction::PlacingClone => {
                                *interaction = Interaction::None;
                                return Some(
                                    canvas::Action::publish(Message::ClonePlacementConfirm(
                                        canvas_pos,
                                    ))
                                    .and_capture(),
                                );
                            }
                            Interaction::Drawing {
                                tool,
                                state: draw_state,
                            } => {
                                return drawing::handle_drawing_click(
                                    interaction,
                                    tool,
                                    draw_state,
                                    canvas_pos,
                                    chart_state.shift_held,
                                );
                            }
                            Interaction::None
                            | Interaction::SelectedLockedDrawing { .. }
                            | Interaction::Panning { .. }
                            | Interaction::Zoomin { .. }
                            | Interaction::Decelerating { .. } => {
                                // Hit-test study overlay labels
                                if let Some(idx) = chart.hit_test_study_overlay(canvas_pos) {
                                    use std::time::Instant;
                                    let now = Instant::now();
                                    let is_double = chart_state
                                        .last_study_overlay_click
                                        .is_some_and(|(t, prev_idx)| {
                                            prev_idx == idx
                                                && now.duration_since(t).as_millis() < 400
                                        });

                                    if is_double {
                                        chart_state.last_study_overlay_click = None;
                                        return Some(
                                            canvas::Action::publish(
                                                Message::StudyOverlayDoubleClick(idx),
                                            )
                                            .and_capture(),
                                        );
                                    }

                                    chart_state.last_study_overlay_click = Some((now, idx));
                                    chart_state.selected_study_overlay = Some(idx);
                                    return Some(
                                        canvas::Action::publish(Message::StudyOverlaySelect(idx))
                                            .and_capture(),
                                    );
                                }

                                // Clear study overlay selection on click elsewhere
                                chart_state.selected_study_overlay = None;

                                // Try entering drawing mode first
                                if let Some(action) = drawing::handle_enter_drawing(
                                    chart,
                                    interaction,
                                    canvas_pos,
                                    chart_state.shift_held,
                                ) {
                                    return Some(action);
                                }

                                // Try selection/editing click
                                if let Some(action) = drawing::handle_selection_click(
                                    chart,
                                    interaction,
                                    canvas_pos,
                                    &mut chart_state.last_selection_click,
                                ) {
                                    return Some(action);
                                }

                                // Otherwise, pan as before (use shrunken-bounds
                                // cursor for panning - deltas cancel out)
                                chart_state.prev_pan_cursor = Some(cursor_in_bounds);
                                chart_state.pan_velocity = Vector::ZERO;
                                *interaction = Interaction::Panning {
                                    translation: state.translation,
                                    start: cursor_in_bounds,
                                };
                            }
                            Interaction::Ruler { start } => {
                                ruler::handle_ruler_click(interaction, start, cursor_in_bounds);
                            }
                            Interaction::EditingDrawing { .. } => {
                                // Already editing, ignore
                            }
                        }
                    }
                    Some(canvas::Action::request_redraw().and_capture())
                }
                mouse::Event::CursorMoved { .. } => match *interaction {
                    Interaction::PlacingClone => canvas_cursor.map(|cursor_pos| {
                        canvas::Action::publish(Message::ClonePlacementMove(cursor_pos))
                            .and_capture()
                    }),
                    Interaction::Panning { translation, start } => {
                        let cursor_in_bounds = cursor_position?;

                        // Track velocity using exponential smoothing
                        if let Some(prev) = chart_state.prev_pan_cursor {
                            let delta = Vector::new(
                                cursor_in_bounds.x - prev.x,
                                cursor_in_bounds.y - prev.y,
                            );
                            // Exponential moving average (alpha = 0.3)
                            chart_state.pan_velocity = Vector::new(
                                chart_state.pan_velocity.x * 0.7 + delta.x * 0.3,
                                chart_state.pan_velocity.y * 0.7 + delta.y * 0.3,
                            );
                        }
                        chart_state.prev_pan_cursor = Some(cursor_in_bounds);

                        pan_zoom::handle_panning(chart, translation, start, cursor_in_bounds)
                    }
                    Interaction::Drawing { .. } => {
                        drawing::handle_drawing_move(chart, canvas_cursor, chart_state.shift_held)
                    }
                    Interaction::EditingDrawing {
                        edit_mode,
                        last_screen_pos,
                        ref mut drag_committed,
                        ..
                    } => drawing::handle_editing_move(
                        edit_mode,
                        canvas_cursor,
                        chart_state.shift_held,
                        last_screen_pos,
                        drag_committed,
                    ),
                    Interaction::SelectedLockedDrawing { press_pos } => {
                        let cursor_in_bounds = match cursor_position {
                            Some(p) => p,
                            None => {
                                return Some(canvas::Action::publish(Message::CrosshairMoved(
                                    cursor_position,
                                )));
                            }
                        };
                        chart_state.prev_pan_cursor = Some(cursor_in_bounds);
                        chart_state.pan_velocity = Vector::ZERO;
                        let state = chart.state();
                        *interaction = Interaction::Panning {
                            translation: state.translation,
                            start: press_pos,
                        };
                        pan_zoom::handle_panning(
                            chart,
                            state.translation,
                            press_pos,
                            cursor_in_bounds,
                        )
                    }
                    Interaction::None | Interaction::Ruler { .. } => Some(canvas::Action::publish(
                        Message::CrosshairMoved(cursor_position),
                    )),
                    _ => None,
                },
                mouse::Event::WheelScrolled { delta } => {
                    cursor_position?;

                    let cursor_to_center = cursor.position_from(bounds.center())?;

                    pan_zoom::handle_scroll_zoom(
                        chart,
                        interaction,
                        delta,
                        cursor_to_center,
                        chart_state.shift_held,
                    )
                }
                _ => None,
            }
        }
        canvas::Event::Keyboard(keyboard_event) => {
            cursor_position?;
            match keyboard_event {
                iced::keyboard::Event::KeyPressed { key, .. } => match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::Shift) => {
                        chart_state.shift_held = true;
                        let interaction = &mut chart_state.interaction;
                        // Enter ruler mode only when not drawing, editing,
                        // or placing a clone. Don't reset if already in ruler
                        // mode (key repeat would clear the start point).
                        if !interaction.is_drawing()
                            && !interaction.is_editing_drawing()
                            && !matches!(
                                interaction,
                                Interaction::Ruler { .. } | Interaction::PlacingClone
                            )
                        {
                            *interaction = Interaction::Ruler { start: None };
                        }
                        Some(canvas::Action::request_redraw().and_capture())
                    }
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        let interaction = &mut chart_state.interaction;
                        if matches!(interaction, Interaction::PlacingClone) {
                            *interaction = Interaction::None;
                            return Some(
                                canvas::Action::publish(Message::ClonePlacementCancel)
                                    .and_capture(),
                            );
                        }
                        if interaction.is_drawing() {
                            *interaction = Interaction::None;
                            return Some(
                                canvas::Action::publish(Message::DrawingCancel).and_capture(),
                            );
                        }
                        *interaction = Interaction::None;
                        Some(canvas::Action::request_redraw().and_capture())
                    }
                    keyboard::Key::Named(keyboard::key::Named::Delete)
                    | keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                        Some(canvas::Action::publish(Message::DrawingDelete).and_capture())
                    }
                    _ => None,
                },
                iced::keyboard::Event::KeyReleased { key, .. } => match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::Shift) => {
                        chart_state.shift_held = false;
                        if matches!(chart_state.interaction, Interaction::Ruler { .. }) {
                            chart_state.interaction = Interaction::None;
                        }
                        // Always redraw to update constraint preview
                        Some(canvas::Action::request_redraw())
                    }
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}
