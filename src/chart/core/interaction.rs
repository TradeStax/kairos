//! User Interaction State
//!
//! Handles mouse and keyboard interactions with the chart canvas.

use super::traits::Chart;
use crate::chart::Message;
use crate::component::layout::multi_split::DRAG_SIZE;
use data::DrawingTool;
use crate::style::tokens;
use iced::{Point, Rectangle, Vector, keyboard, mouse, widget::canvas};

const ZOOM_SENSITIVITY: f32 = tokens::chart::ZOOM_SENSITIVITY;
const ZOOM_BASE: f32 = tokens::chart::ZOOM_BASE;

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
    /// Editing an existing drawing
    EditingDrawing {
        /// The edit operation in progress
        edit_mode: DrawingEditMode,
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
    Moving {
        /// Offset from cursor to drawing anchor
        offset: Vector,
    },
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

/// Process canvas interaction events and produce chart messages
///
/// Handles mouse events (pan, zoom, click) and keyboard events (ruler mode, escape).
/// Returns appropriate canvas action with message if interaction occurred.
pub fn canvas_interaction<T: Chart>(
    chart: &T,
    interaction: &mut Interaction,
    event: &canvas::Event,
    bounds: Rectangle,
    cursor: mouse::Cursor,
) -> Option<canvas::Action<Message>> {
    // Check for bounds change
    if chart.state().bounds != bounds {
        return Some(canvas::Action::publish(Message::BoundsChanged(bounds)));
    }

    let shrunken_bounds = bounds.shrink(DRAG_SIZE * 4.0);
    let cursor_position = cursor.position_in(shrunken_bounds);

    // Handle button release - end active interactions
    if let canvas::Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
        match interaction {
            Interaction::Panning { .. } | Interaction::Zoomin { .. } => {
                *interaction = Interaction::None;
            }
            Interaction::EditingDrawing { .. } => {
                *interaction = Interaction::None;
            }
            _ => {}
        }
    }

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
                    let cursor_in_bounds = cursor_position?;

                    if let mouse::Button::Left = button {
                        match interaction {
                            // Drawing mode interactions handled by chart (via DrawingManager)
                            Interaction::Drawing { tool, state: draw_state } => {
                                match draw_state {
                                    DrawingState::Placing => {
                                        // First click - start the drawing
                                        *interaction = Interaction::Drawing {
                                            tool: *tool,
                                            state: DrawingState::Previewing { start: cursor_in_bounds },
                                        };
                                        // Emit message to create the drawing
                                        return Some(canvas::Action::publish(Message::DrawingClick(cursor_in_bounds)).and_capture());
                                    }
                                    DrawingState::Previewing { .. } => {
                                        // Second click - complete the drawing
                                        // Return to placing state for next drawing
                                        *interaction = Interaction::Drawing {
                                            tool: *tool,
                                            state: DrawingState::Placing,
                                        };
                                        // Emit message to complete the drawing
                                        return Some(canvas::Action::publish(Message::DrawingClick(cursor_in_bounds)).and_capture());
                                    }
                                }
                            }
                            Interaction::None
                            | Interaction::Panning { .. }
                            | Interaction::Zoomin { .. } => {
                                // Check if a drawing tool is active
                                let active_tool = chart.active_drawing_tool();
                                if active_tool != DrawingTool::None {
                                    // Enter drawing mode and emit click to start drawing
                                    *interaction = Interaction::Drawing {
                                        tool: active_tool,
                                        state: DrawingState::Previewing {
                                            start: cursor_in_bounds,
                                        },
                                    };
                                    return Some(
                                        canvas::Action::publish(Message::DrawingClick(
                                            cursor_in_bounds,
                                        ))
                                        .and_capture(),
                                    );
                                }

                                // Otherwise, pan as before
                                *interaction = Interaction::Panning {
                                    translation: state.translation,
                                    start: cursor_in_bounds,
                                };
                            }
                            Interaction::Ruler { start } if start.is_none() => {
                                *interaction = Interaction::Ruler {
                                    start: Some(cursor_in_bounds),
                                };
                            }
                            Interaction::Ruler { .. } => {
                                *interaction = Interaction::None;
                            }
                            Interaction::EditingDrawing { .. } => {
                                // Already editing, let the chart handle it
                            }
                        }
                    }
                    Some(canvas::Action::request_redraw().and_capture())
                }
                mouse::Event::CursorMoved { .. } => match *interaction {
                    Interaction::Panning { translation, start } => {
                        let cursor_in_bounds = cursor_position?;
                        let msg = Message::Translated(
                            translation + (cursor_in_bounds - start) * (1.0 / state.scaling),
                        );
                        Some(canvas::Action::publish(msg).and_capture())
                    }
                    Interaction::Drawing { .. } => {
                        // Cursor move during drawing mode - emit move event for preview
                        if let Some(cursor_pos) = cursor_position {
                            Some(canvas::Action::publish(Message::DrawingMove(cursor_pos)))
                        } else {
                            Some(canvas::Action::publish(Message::CrosshairMoved))
                        }
                    }
                    Interaction::EditingDrawing { .. } => {
                        // Cursor move during editing - redraw
                        Some(canvas::Action::publish(Message::CrosshairMoved))
                    }
                    Interaction::None | Interaction::Ruler { .. } => {
                        Some(canvas::Action::publish(Message::CrosshairMoved))
                    }
                    _ => None,
                },
                mouse::Event::WheelScrolled { delta } => {
                    cursor_position?;

                    let default_cell_width = T::default_cell_width(chart);
                    let min_cell_width = T::min_cell_width(chart);
                    let max_cell_width = T::max_cell_width(chart);
                    let max_scaling = T::max_scaling(chart);
                    let min_scaling = T::min_scaling(chart);

                    if matches!(interaction, Interaction::Panning { .. }) {
                        return Some(canvas::Action::capture());
                    }

                    let cursor_to_center = cursor.position_from(bounds.center())?;
                    let y = match delta {
                        mouse::ScrollDelta::Lines { y, .. }
                        | mouse::ScrollDelta::Pixels { y, .. } => y,
                    };

                    // Handle fit-all autoscale mode
                    if let Some(data::Autoscale::FitAll) = state.layout.autoscale {
                        return Some(
                            canvas::Action::publish(Message::XScaling(
                                y / 2.0,
                                cursor_to_center.x,
                                false,
                            ))
                            .and_capture(),
                        );
                    }

                    // Determine if we should adjust cell width instead of scaling
                    let should_adjust_cell_width = match (y.signum(), state.scaling) {
                        (-1.0, scaling)
                            if scaling == max_scaling && state.cell_width > default_cell_width =>
                        {
                            true
                        }
                        (1.0, scaling)
                            if scaling == min_scaling && state.cell_width < default_cell_width =>
                        {
                            true
                        }
                        (1.0, scaling)
                            if scaling == max_scaling && state.cell_width < max_cell_width =>
                        {
                            true
                        }
                        (-1.0, scaling)
                            if scaling == min_scaling && state.cell_width > min_cell_width =>
                        {
                            true
                        }
                        _ => false,
                    };

                    if should_adjust_cell_width {
                        return Some(
                            canvas::Action::publish(Message::XScaling(
                                y / 2.0,
                                cursor_to_center.x,
                                true,
                            ))
                            .and_capture(),
                        );
                    }

                    // Normal scaling cases
                    if (*y < 0.0 && state.scaling > min_scaling)
                        || (*y > 0.0 && state.scaling < max_scaling)
                    {
                        let old_scaling = state.scaling;
                        let scaling = (state.scaling * ZOOM_BASE.powf(y / ZOOM_SENSITIVITY))
                            .clamp(min_scaling, max_scaling);

                        let denominator = old_scaling * scaling;
                        let vector_diff = if denominator.abs() > 0.0001 {
                            let factor = scaling - old_scaling;
                            Vector::new(
                                cursor_to_center.x * factor / denominator,
                                cursor_to_center.y * factor / denominator,
                            )
                        } else {
                            Vector::default()
                        };

                        let translation = state.translation - vector_diff;

                        return Some(
                            canvas::Action::publish(Message::Scaled(scaling, translation))
                                .and_capture(),
                        );
                    }

                    Some(canvas::Action::capture())
                }
                _ => None,
            }
        }
        canvas::Event::Keyboard(keyboard_event) => {
            cursor_position?;
            match keyboard_event {
                iced::keyboard::Event::KeyPressed { key, .. } => match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::Shift) => {
                        // Don't enter ruler mode if we're drawing
                        if !interaction.is_drawing() {
                            *interaction = Interaction::Ruler { start: None };
                        }
                        Some(canvas::Action::request_redraw().and_capture())
                    }
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        if interaction.is_drawing() {
                            // Cancel pending drawing
                            return Some(canvas::Action::publish(Message::DrawingCancel).and_capture());
                        }
                        *interaction = Interaction::None;
                        Some(canvas::Action::request_redraw().and_capture())
                    }
                    keyboard::Key::Named(keyboard::key::Named::Delete)
                    | keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                        // Delete key pressed - emit delete message
                        Some(canvas::Action::publish(Message::DrawingDelete).and_capture())
                    }
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}
