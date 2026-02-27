use crate::chart;
use crate::screen::dashboard::pane::{Action, Content, State};

impl State {
    /// Handle chart interaction events.
    /// Returns `Some(Action)` if a side-effect should bubble up.
    pub(super) fn handle_chart_interaction(&mut self, msg: chart::Message) -> Option<Action> {
        match msg {
            chart::Message::StudyOverlaySelect(_) => {
                // Selection state lives in ChartState; no-op here
            }
            chart::Message::StudyOverlayDoubleClick(idx) => {
                self.open_indicator_manager_for_study(idx);
            }
            chart::Message::StudyOverlayContextMenu(pos, idx) => {
                use crate::screen::dashboard::pane::ContextMenuKind;

                self.modal = None;
                self.context_menu = Some(ContextMenuKind::StudyOverlay {
                    position: pos,
                    study_index: idx,
                });
            }
            chart::Message::StudyDetailClick(idx) => {
                self.open_level_detail_modal(idx);
            }
            chart::Message::DrawingClick(point, shift_held) => {
                if self.handle_drawing_click(point, shift_held) {
                    return Some(Action::DrawingToolChanged(
                        crate::drawing::DrawingTool::None,
                    ));
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
                let effective_id = drawing_id.or_else(|| self.get_selected_drawing_id());

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
            chart::Message::CrosshairMoved(cursor_pos) => {
                // Optimize crosshair updates when a drawing tool is active:
                // Only clear the main crosshair cache, skip indicator caches
                use crate::chart::Chart;
                match &mut self.content {
                    Content::Candlestick { chart, .. } => {
                        if let Some(c) = (**chart).as_mut() {
                            if c.drawings().active_tool() != crate::drawing::DrawingTool::None {
                                c.mut_state().cache.clear_crosshair();
                            } else {
                                chart::update(c, &msg);
                            }
                        }
                    }
                    #[cfg(feature = "heatmap")]
                    Content::Heatmap { chart: Some(c), .. } => {
                        if c.drawings().active_tool() != crate::drawing::DrawingTool::None {
                            c.mut_state().cache.clear_crosshair();
                        } else {
                            chart::update(c, &msg);
                        }
                    }
                    Content::Profile { chart, .. } => {
                        if let Some(c) = (**chart).as_mut() {
                            if c.drawings().active_tool() != crate::drawing::DrawingTool::None {
                                c.mut_state().cache.clear_crosshair();
                            } else {
                                chart::update(c, &msg);
                            }
                        }
                    }
                    _ => {}
                }

                // Emit crosshair sync for linked panes
                if self.link_group.is_some() {
                    let interval = cursor_pos.and_then(|pos| self.compute_crosshair_interval(pos));
                    return Some(Action::CrosshairSync {
                        timestamp: interval,
                    });
                }
            }
            chart::Message::CursorLeft => {
                match &mut self.content {
                    Content::Candlestick { chart, .. } => {
                        if let Some(c) = (**chart).as_mut() {
                            chart::update(c, &msg);
                        }
                    }
                    #[cfg(feature = "heatmap")]
                    Content::Heatmap { chart: Some(c), .. } => {
                        chart::update(c, &msg);
                    }
                    Content::Profile { chart, .. } => {
                        if let Some(c) = (**chart).as_mut() {
                            chart::update(c, &msg);
                        }
                    }
                    _ => {}
                }

                if self.link_group.is_some() {
                    return Some(Action::CrosshairSync { timestamp: None });
                }
            }
            _ => match &mut self.content {
                #[cfg(feature = "heatmap")]
                Content::Heatmap { chart: Some(c), .. } => {
                    chart::update(c, &msg);
                }
                Content::Candlestick { chart, .. } => {
                    if let Some(c) = (**chart).as_mut() {
                        chart::update(c, &msg);
                    }
                }
                Content::Profile { chart, .. } => {
                    if let Some(c) = (**chart).as_mut() {
                        chart::update(c, &msg);
                    }
                }
                _ => {}
            },
        }
        None
    }

    /// Compute the snapped interval (timestamp or tick index) for a cursor
    /// position within the chart bounds.
    fn compute_crosshair_interval(&self, pos: iced::Point) -> Option<u64> {
        use crate::chart::Chart;
        let state = match &self.content {
            Content::Candlestick { chart, .. } => {
                if let Some(c) = (**chart).as_ref() {
                    c.state()
                } else {
                    return None;
                }
            }
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart: Some(c), .. } => c.state(),
            Content::Profile { chart, .. } => {
                if let Some(c) = (**chart).as_ref() {
                    c.state()
                } else {
                    return None;
                }
            }
            _ => return None,
        };

        let bounds = state.bounds.size();
        if bounds.width < f32::EPSILON || bounds.height < f32::EPSILON {
            return None;
        }

        let region = state.visible_region(bounds);
        let (interval, _snap_ratio) = state.snap_x_to_index(pos.x, bounds, region);
        Some(interval)
    }
}
