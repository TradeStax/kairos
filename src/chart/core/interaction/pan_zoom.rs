//! Pan and zoom interaction handlers

use super::Interaction;
use crate::chart::Message;
use crate::chart::core::traits::Chart;
use iced::{Point, Vector, mouse, widget::canvas};

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
///
/// Scroll wheel always zooms the X-axis (time) via cell_width adjustment.
/// FitAll mode anchors zoom to the latest visible candle and preserves autoscale.
/// Other modes use cursor-anchored zoom.
pub fn handle_scroll_zoom<T: Chart>(
    chart: &T,
    interaction: &Interaction,
    delta: &mouse::ScrollDelta,
    cursor_to_center: Point,
) -> Option<canvas::Action<Message>> {
    if matches!(interaction, Interaction::Panning { .. }) {
        return Some(canvas::Action::capture());
    }

    let y = match delta {
        mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => y,
    };

    let state = chart.state();

    // FitAll: anchor to latest visible candle, preserve autoscale (is_wheel_scroll=false)
    // Other modes: cursor-anchored zoom (is_wheel_scroll=true)
    let is_wheel_scroll = !matches!(state.layout.autoscale, Some(data::Autoscale::FitAll));

    Some(
        canvas::Action::publish(Message::XScaling(y / 2.0, cursor_to_center.x, is_wheel_scroll))
            .and_capture(),
    )
}
