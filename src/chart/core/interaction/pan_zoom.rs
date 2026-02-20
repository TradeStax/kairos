//! Pan and zoom interaction handlers

use super::Interaction;
use crate::chart::Message;
use crate::chart::core::traits::Chart;
use crate::style::tokens;
use iced::{Point, Vector, mouse, widget::canvas};

const ZOOM_SENSITIVITY: f32 = tokens::chart::ZOOM_SENSITIVITY;
const ZOOM_BASE: f32 = tokens::chart::ZOOM_BASE;

/// Handle cursor movement during panning
pub fn handle_panning<T: Chart>(
    chart: &T,
    translation: Vector,
    start: Point,
    cursor_position: Point,
) -> Option<canvas::Action<Message>> {
    let state = chart.state();
    let msg = Message::Translated(translation + (cursor_position - start) * (1.0 / state.scaling));
    Some(canvas::Action::publish(msg).and_capture())
}

/// Handle scroll wheel zoom events
pub fn handle_scroll_zoom<T: Chart>(
    chart: &T,
    interaction: &Interaction,
    delta: &mouse::ScrollDelta,
    cursor_to_center: Point,
) -> Option<canvas::Action<Message>> {
    let state = chart.state();

    let default_cell_width = T::default_cell_width(chart);
    let min_cell_width = T::min_cell_width(chart);
    let max_cell_width = T::max_cell_width(chart);
    let max_scaling = T::max_scaling(chart);
    let min_scaling = T::min_scaling(chart);

    if matches!(interaction, Interaction::Panning { .. }) {
        return Some(canvas::Action::capture());
    }

    let y = match delta {
        mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => y,
    };

    // Handle fit-all autoscale mode
    if let Some(data::Autoscale::FitAll) = state.layout.autoscale {
        return Some(
            canvas::Action::publish(Message::XScaling(y / 2.0, cursor_to_center.x, false))
                .and_capture(),
        );
    }

    // Determine if we should adjust cell width instead of scaling
    let should_adjust_cell_width = match (y.signum(), state.scaling) {
        (-1.0, scaling) if scaling == max_scaling && state.cell_width > default_cell_width => true,
        (1.0, scaling) if scaling == min_scaling && state.cell_width < default_cell_width => true,
        (1.0, scaling) if scaling == max_scaling && state.cell_width < max_cell_width => true,
        (-1.0, scaling) if scaling == min_scaling && state.cell_width > min_cell_width => true,
        _ => false,
    };

    if should_adjust_cell_width {
        return Some(
            canvas::Action::publish(Message::XScaling(y / 2.0, cursor_to_center.x, true))
                .and_capture(),
        );
    }

    // Normal scaling cases
    if (*y < 0.0 && state.scaling > min_scaling) || (*y > 0.0 && state.scaling < max_scaling) {
        let old_scaling = state.scaling;
        let scaling =
            (state.scaling * ZOOM_BASE.powf(y / ZOOM_SENSITIVITY)).clamp(min_scaling, max_scaling);

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

        return Some(canvas::Action::publish(Message::Scaled(scaling, translation)).and_capture());
    }

    Some(canvas::Action::capture())
}
