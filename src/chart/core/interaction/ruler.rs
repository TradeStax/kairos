//! Ruler interaction handlers

use super::Interaction;
use iced::Point;

/// Handle left click during Ruler state
pub fn handle_ruler_click(
    interaction: &mut Interaction,
    _start: Option<Point>,
    cursor_in_bounds: Point,
) {
    // Always (re)set the start point on click — ruler stays active
    // until Shift is released
    *interaction = Interaction::Ruler {
        start: Some(cursor_in_bounds),
    };
}
