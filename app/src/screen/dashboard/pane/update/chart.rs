use crate::chart;
use crate::screen::dashboard::pane::{Content, Effect, State};

impl State {
    /// Handle chart interaction events.
    /// Returns `Some(Effect)` if a side-effect should bubble up.
    pub(super) fn handle_chart_interaction(
        &mut self,
        msg: chart::Message,
    ) -> Option<Effect> {
        match msg {
            chart::Message::IndicatorClicked(_) => {
                // Indicator panels removed; no-op
            }
            chart::Message::DrawingClick(point, shift_held) => {
                if self.handle_drawing_click(point, shift_held) {
                    return Some(Effect::DrawingToolChanged(data::DrawingTool::None));
                }
            }
            chart::Message::DrawingMove(point, shift_held) => {
                self.handle_drawing_move(point, shift_held);
            }
            chart::Message::ClonePlacementMove(point) => {
                self.handle_clone_move(point);
            }
            chart::Message::ClonePlacementConfirm(point) => {
                self.handle_clone_confirm(point);
            }
            chart::Message::ClonePlacementCancel => {
                self.handle_clone_cancel();
            }
            chart::Message::DrawingCancel => {
                self.handle_drawing_cancel();
            }
            chart::Message::DrawingDelete => {
                self.handle_drawing_delete();
            }
            chart::Message::DrawingSelect(id) => {
                self.handle_drawing_select(id);
            }
            chart::Message::DrawingDeselect => {
                self.handle_drawing_deselect();
            }
            chart::Message::DrawingDrag(point, shift_held) => {
                self.handle_drawing_drag(point, shift_held);
            }
            chart::Message::DrawingHandleDrag(point, handle_index, shift_held) => {
                self.handle_drawing_handle_drag(point, handle_index, shift_held);
            }
            chart::Message::DrawingDragEnd => {
                self.handle_drawing_drag_end();
            }
            chart::Message::DrawingDoubleClick(id) => {
                self.handle_open_drawing_properties(id);
            }
            chart::Message::ContextMenu(position, drawing_id) => {
                use crate::screen::dashboard::pane::ContextMenuKind;

                self.modal = None;

                // Use hit-tested drawing, or fall back to
                // currently selected drawing
                let effective_id =
                    drawing_id.or_else(|| self.get_selected_drawing_id());

                if let Some(id) = effective_id {
                    let locked = self.get_drawing_locked(id);
                    self.context_menu = Some(ContextMenuKind::Drawing {
                        position,
                        id,
                        locked,
                    });
                } else {
                    self.context_menu = Some(ContextMenuKind::Chart { position });
                }
            }
            chart::Message::CrosshairMoved => {
                // Optimize crosshair updates when a drawing tool is active:
                // Only clear the main crosshair cache, skip indicator caches
                use crate::chart::Chart;
                match &mut self.content {
                    Content::Kline { chart: Some(c), .. } => {
                        if c.drawings().active_tool() != data::DrawingTool::None {
                            c.mut_state().cache.clear_crosshair();
                        } else {
                            chart::update(c, &msg);
                        }
                    }
                    Content::Heatmap { chart: Some(c), .. } => {
                        if c.drawings().active_tool() != data::DrawingTool::None {
                            c.mut_state().cache.clear_crosshair();
                        } else {
                            chart::update(c, &msg);
                        }
                    }
                    Content::Profile { chart: Some(c), .. } => {
                        if c.drawings().active_tool() != data::DrawingTool::None {
                            c.mut_state().cache.clear_crosshair();
                        } else {
                            chart::update(c, &msg);
                        }
                    }
                    _ => {}
                }
            }
            _ => match &mut self.content {
                Content::Heatmap { chart: Some(c), .. } => {
                    chart::update(c, &msg);
                }
                Content::Kline { chart: Some(c), .. } => {
                    chart::update(c, &msg);
                }
                Content::Profile { chart: Some(c), .. } => {
                    chart::update(c, &msg);
                }
                _ => {}
            },
        }
        None
    }
}
