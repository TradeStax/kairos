use super::{Content, State};
use crate::chart::{Chart, drawing::DrawingPoint};

impl State {
    /// Handle drawing click at a screen position
    pub(super) fn handle_drawing_click(
        &mut self,
        screen_point: iced::Point,
    ) {
        match &mut self.content {
            Content::Kline {
                chart: Some(c), ..
            } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let point = DrawingPoint::from_screen(
                    screen_point,
                    state,
                    bounds,
                    c.drawings.snap_enabled(),
                );

                if c.drawings.has_pending() {
                    c.drawings.complete_drawing(point);
                } else {
                    c.drawings.start_drawing(point);
                }
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap {
                chart: Some(c), ..
            } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let point = DrawingPoint::from_screen(
                    screen_point,
                    state,
                    bounds,
                    c.drawings.snap_enabled(),
                );

                if c.drawings.has_pending() {
                    c.drawings.complete_drawing(point);
                } else {
                    c.drawings.start_drawing(point);
                }
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Handle drawing move (update preview)
    pub(super) fn handle_drawing_move(
        &mut self,
        screen_point: iced::Point,
    ) {
        match &mut self.content {
            Content::Kline {
                chart: Some(c), ..
            } => {
                if c.drawings.has_pending() {
                    let state = c.state();
                    let bounds = state.bounds.size();
                    let point = DrawingPoint::from_screen(
                        screen_point,
                        state,
                        bounds,
                        c.drawings.snap_enabled(),
                    );
                    c.drawings.update_preview(point);
                    c.mut_state().cache.clear_crosshair();
                }
            }
            Content::Heatmap {
                chart: Some(c), ..
            } => {
                if c.drawings.has_pending() {
                    let state = c.state();
                    let bounds = state.bounds.size();
                    let point = DrawingPoint::from_screen(
                        screen_point,
                        state,
                        bounds,
                        c.drawings.snap_enabled(),
                    );
                    c.drawings.update_preview(point);
                    c.mut_state().cache.clear_crosshair();
                }
            }
            _ => {}
        }
    }

    /// Handle drawing cancel (Escape key)
    pub(super) fn handle_drawing_cancel(&mut self) {
        match &mut self.content {
            Content::Kline {
                chart: Some(c), ..
            } => {
                c.drawings.cancel_pending();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap {
                chart: Some(c), ..
            } => {
                c.drawings.cancel_pending();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Handle drawing delete (Delete/Backspace key)
    pub(super) fn handle_drawing_delete(&mut self) {
        match &mut self.content {
            Content::Kline {
                chart: Some(c), ..
            } => {
                c.drawings.delete_selected();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap {
                chart: Some(c), ..
            } => {
                c.drawings.delete_selected();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }
}
