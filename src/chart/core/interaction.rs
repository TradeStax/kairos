//! User Interaction State
//!
//! Handles mouse and keyboard interactions with the chart canvas.

use super::traits::{Chart, PlotConstants};
use crate::chart::Message;
use crate::widget::multi_split::DRAG_SIZE;
use iced::{Point, Rectangle, Vector, keyboard, mouse, widget::canvas};

const ZOOM_SENSITIVITY: f32 = 30.0;
const ZOOM_BASE: f32 = 2.0;

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
                            Interaction::None
                            | Interaction::Panning { .. }
                            | Interaction::Zoomin { .. } => {
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
                        *interaction = Interaction::Ruler { start: None };
                        Some(canvas::Action::request_redraw().and_capture())
                    }
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        *interaction = Interaction::None;
                        Some(canvas::Action::request_redraw().and_capture())
                    }
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}
