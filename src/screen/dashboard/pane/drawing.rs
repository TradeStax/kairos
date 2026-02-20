use super::{Content, Effect, State};
use crate::chart::Chart;
use crate::chart::ViewState;
use crate::chart::drawing::{Drawing, DrawingManager, DrawingPoint, snap};
use data::{
    ChartBasis, ChartConfig, ContentKind, DataSchema, DateRange, DrawingId, DrawingTool,
    LoadingStatus, Timeframe,
};
use iced::Size;

/// Get the anchor point in screen coords for shift-constrained creation.
/// Returns `(tool, anchor_screen_point)` if a pending drawing has an anchor.
fn creation_anchor(
    drawings: &DrawingManager,
    state: &ViewState,
    bounds: Size,
) -> Option<(DrawingTool, iced::Point)> {
    let pending = drawings.pending()?;
    if pending.points.is_empty() {
        return None;
    }
    Some((pending.tool, pending.points[0].to_screen(state, bounds)))
}

impl State {
    /// Handle drawing click at a screen position.
    /// Returns `true` if a drawing was completed and the tool was auto-switched
    /// to selection mode.
    pub(super) fn handle_drawing_click(
        &mut self,
        screen_point: iced::Point,
        shift_held: bool,
    ) -> bool {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let snap = c.drawings.snap_enabled();

                // Apply shift constraint when completing (has anchor)
                let constrained = if shift_held {
                    creation_anchor(&c.drawings, state, bounds)
                        .map(|(tool, anchor)| snap::constrain_creation(tool, anchor, screen_point))
                        .unwrap_or(screen_point)
                } else {
                    screen_point
                };

                let point = DrawingPoint::from_screen(constrained, state, bounds, snap);

                let completed_id = if c.drawings.has_pending() {
                    c.drawings.complete_drawing(point)
                } else {
                    c.drawings.start_drawing(point)
                };

                if let Some(id) = completed_id {
                    c.drawings.set_tool(DrawingTool::None);
                    c.drawings.select(id);
                    c.mut_state().cache.clear_drawings();
                    c.mut_state().cache.clear_crosshair();
                    return true;
                }
                c.mut_state().cache.clear_crosshair();
                false
            }
            Content::Heatmap { chart: Some(c), .. } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let snap = c.drawings.snap_enabled();

                let constrained = if shift_held {
                    creation_anchor(&c.drawings, state, bounds)
                        .map(|(tool, anchor)| snap::constrain_creation(tool, anchor, screen_point))
                        .unwrap_or(screen_point)
                } else {
                    screen_point
                };

                let point = DrawingPoint::from_screen(constrained, state, bounds, snap);

                let completed_id = if c.drawings.has_pending() {
                    c.drawings.complete_drawing(point)
                } else {
                    c.drawings.start_drawing(point)
                };

                if let Some(id) = completed_id {
                    c.drawings.set_tool(DrawingTool::None);
                    c.drawings.select(id);
                    c.mut_state().cache.clear_drawings();
                    c.mut_state().cache.clear_crosshair();
                    return true;
                }
                c.mut_state().cache.clear_crosshair();
                false
            }
            _ => false,
        }
    }

    /// Handle drawing move (update preview)
    pub(super) fn handle_drawing_move(&mut self, screen_point: iced::Point, shift_held: bool) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                if c.drawings.has_pending() {
                    let state = c.state();
                    let bounds = state.bounds.size();
                    let snap = c.drawings.snap_enabled();

                    let constrained = if shift_held {
                        creation_anchor(&c.drawings, state, bounds)
                            .map(|(tool, anchor)| {
                                snap::constrain_creation(tool, anchor, screen_point)
                            })
                            .unwrap_or(screen_point)
                    } else {
                        screen_point
                    };

                    let point = DrawingPoint::from_screen(constrained, state, bounds, snap);
                    c.drawings.update_preview(point);
                    c.mut_state().cache.clear_crosshair();
                }
            }
            Content::Heatmap { chart: Some(c), .. } => {
                if c.drawings.has_pending() {
                    let state = c.state();
                    let bounds = state.bounds.size();
                    let snap = c.drawings.snap_enabled();

                    let constrained = if shift_held {
                        creation_anchor(&c.drawings, state, bounds)
                            .map(|(tool, anchor)| {
                                snap::constrain_creation(tool, anchor, screen_point)
                            })
                            .unwrap_or(screen_point)
                    } else {
                        screen_point
                    };

                    let point = DrawingPoint::from_screen(constrained, state, bounds, snap);
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
            Content::Kline { chart: Some(c), .. } => {
                c.drawings.cancel_pending();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.cancel_pending();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Handle drawing delete (Delete/Backspace key)
    pub(super) fn handle_drawing_delete(&mut self) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                c.drawings.delete_selected();
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.delete_selected();
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Handle drawing selection
    pub(super) fn handle_drawing_select(&mut self, id: DrawingId) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                c.drawings.select(id);
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.select(id);
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Handle drawing deselection
    pub(super) fn handle_drawing_deselect(&mut self) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                c.drawings.clear_selection();
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.clear_selection();
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Handle whole-drawing drag (moving entire drawing)
    pub(super) fn handle_drawing_drag(&mut self, screen_point: iced::Point, shift_held: bool) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let snap = c.drawings.snap_enabled();

                // Axis-lock when shift held
                let constrained = if shift_held {
                    c.drawings
                        .drag_start_screen()
                        .map(|start| snap::constrain_axis(start, screen_point))
                        .unwrap_or(screen_point)
                } else {
                    screen_point
                };

                let point = DrawingPoint::from_screen(constrained, state, bounds, snap);
                let selected: Vec<DrawingId> = c.drawings.selected_ids().iter().copied().collect();

                if let Some(&id) = selected.first() {
                    if c.drawings.is_dragging() {
                        c.drawings.update_drag(id, point);
                    } else {
                        c.drawings.start_drag(point, id, screen_point);
                    }
                }
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let snap = c.drawings.snap_enabled();

                let constrained = if shift_held {
                    c.drawings
                        .drag_start_screen()
                        .map(|start| snap::constrain_axis(start, screen_point))
                        .unwrap_or(screen_point)
                } else {
                    screen_point
                };

                let point = DrawingPoint::from_screen(constrained, state, bounds, snap);
                let selected: Vec<DrawingId> = c.drawings.selected_ids().iter().copied().collect();

                if let Some(&id) = selected.first() {
                    if c.drawings.is_dragging() {
                        c.drawings.update_drag(id, point);
                    } else {
                        c.drawings.start_drag(point, id, screen_point);
                    }
                }
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Handle single-handle drag
    pub(super) fn handle_drawing_handle_drag(
        &mut self,
        screen_point: iced::Point,
        handle_index: usize,
        shift_held: bool,
    ) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let snap = c.drawings.snap_enabled();
                let selected: Vec<DrawingId> = c.drawings.selected_ids().iter().copied().collect();
                let Some(&id) = selected.first() else { return };

                // Apply shift constraint based on tool/handle
                let constrained = if shift_held {
                    c.drawings
                        .get(id)
                        .map(|d| {
                            snap::constrain_handle(
                                d.tool,
                                &d.points,
                                handle_index,
                                state,
                                bounds,
                                screen_point,
                            )
                        })
                        .unwrap_or(screen_point)
                } else {
                    screen_point
                };

                let point = DrawingPoint::from_screen(constrained, state, bounds, snap);

                if !c.drawings.is_dragging() {
                    c.drawings.start_drag(point, id, screen_point);
                }
                c.drawings.update_handle_drag(id, handle_index, point);
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let snap = c.drawings.snap_enabled();
                let selected: Vec<DrawingId> = c.drawings.selected_ids().iter().copied().collect();
                let Some(&id) = selected.first() else { return };

                let constrained = if shift_held {
                    c.drawings
                        .get(id)
                        .map(|d| {
                            snap::constrain_handle(
                                d.tool,
                                &d.points,
                                handle_index,
                                state,
                                bounds,
                                screen_point,
                            )
                        })
                        .unwrap_or(screen_point)
                } else {
                    screen_point
                };

                let point = DrawingPoint::from_screen(constrained, state, bounds, snap);

                if !c.drawings.is_dragging() {
                    c.drawings.start_drag(point, id, screen_point);
                }
                c.drawings.update_handle_drag(id, handle_index, point);
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Handle drawing drag end
    pub(super) fn handle_drawing_drag_end(&mut self) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                c.drawings.end_drag();
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.end_drag();
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Get the first selected drawing ID, if any
    pub(super) fn get_selected_drawing_id(&self) -> Option<DrawingId> {
        match &self.content {
            Content::Kline { chart: Some(c), .. } => {
                c.drawings.selected_ids().iter().next().copied()
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.selected_ids().iter().next().copied()
            }
            _ => None,
        }
    }

    /// Get the locked state for a drawing by ID
    pub(super) fn get_drawing_locked(&self, id: DrawingId) -> bool {
        match &self.content {
            Content::Kline { chart: Some(c), .. } => c.drawings.get(id).map_or(false, |d| d.locked),
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.get(id).map_or(false, |d| d.locked)
            }
            _ => false,
        }
    }

    /// Open the drawing properties modal for the given drawing
    pub(super) fn handle_open_drawing_properties(&mut self, id: DrawingId) {
        use crate::modals::drawing_properties::DrawingPropertiesModal;
        use crate::modals::pane::Modal;

        let modal = match &self.content {
            Content::Kline { chart: Some(c), .. } => c.drawings.get(id).map(|d| {
                let snapshot = d.to_serializable();
                DrawingPropertiesModal::new(
                    d.id,
                    d.tool,
                    &d.style,
                    d.locked,
                    d.visible,
                    d.label.clone(),
                    snapshot,
                )
            }),
            Content::Heatmap { chart: Some(c), .. } => c.drawings.get(id).map(|d| {
                let snapshot = d.to_serializable();
                DrawingPropertiesModal::new(
                    d.id,
                    d.tool,
                    &d.style,
                    d.locked,
                    d.visible,
                    d.label.clone(),
                    snapshot,
                )
            }),
            _ => None,
        };

        if let Some(m) = modal {
            self.modal = Some(Modal::DrawingProperties(m));
        }
    }

    /// Apply a drawing style update without recording undo.
    /// Used for live preview and cancel-revert.
    pub(super) fn apply_drawing_style(
        &mut self,
        id: DrawingId,
        update: &crate::modals::drawing_properties::DrawingUpdate,
    ) {
        use crate::chart::Chart;

        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                if let Some(d) = c.drawings.get_mut(id) {
                    d.style = update.style.clone();
                    d.locked = update.locked;
                    d.visible = update.visible;
                    d.label = update.label.clone();
                }
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                if let Some(d) = c.drawings.get_mut(id) {
                    d.style = update.style.clone();
                    d.locked = update.locked;
                    d.visible = update.visible;
                    d.label = update.label.clone();
                }
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Record undo for a property change and sync the default style.
    pub(super) fn finalize_drawing_properties(
        &mut self,
        id: DrawingId,
        before_snapshot: data::SerializableDrawing,
    ) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                c.drawings.record_property_change(id, before_snapshot);
                if let Some(d) = c.drawings.get(id) {
                    c.drawings.set_default_style(d.style.clone());
                }
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.record_property_change(id, before_snapshot);
                if let Some(d) = c.drawings.get(id) {
                    c.drawings.set_default_style(d.style.clone());
                }
            }
            _ => {}
        }
    }

    /// Delete a specific drawing by ID (from context menu)
    pub(super) fn handle_drawing_context_delete(&mut self, id: DrawingId) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                c.drawings.delete(id);
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.delete(id);
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Toggle lock state of a drawing
    pub(super) fn handle_drawing_toggle_lock(&mut self, id: DrawingId) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                if let Some(d) = c.drawings.get_mut(id) {
                    d.locked = !d.locked;
                }
                c.mut_state().cache.clear_drawings();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                if let Some(d) = c.drawings.get_mut(id) {
                    d.locked = !d.locked;
                }
                c.mut_state().cache.clear_drawings();
            }
            _ => {}
        }
    }

    /// Start clone placement mode for a drawing
    pub(super) fn handle_drawing_clone(&mut self, id: DrawingId) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                if let Some(original) = c.drawings.get(id) {
                    let mut cloned = Drawing::from_serializable(&original.to_serializable());
                    cloned.id = DrawingId::new();
                    c.drawings.start_clone_placement(cloned);
                }
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                if let Some(original) = c.drawings.get(id) {
                    let mut cloned = Drawing::from_serializable(&original.to_serializable());
                    cloned.id = DrawingId::new();
                    c.drawings.start_clone_placement(cloned);
                }
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Update clone placement position as cursor moves
    pub(super) fn handle_clone_move(&mut self, screen_point: iced::Point) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let point = DrawingPoint::from_screen(
                    screen_point,
                    state,
                    bounds,
                    c.drawings.snap_enabled(),
                );
                c.drawings.update_clone_position(point);
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let point = DrawingPoint::from_screen(
                    screen_point,
                    state,
                    bounds,
                    c.drawings.snap_enabled(),
                );
                c.drawings.update_clone_position(point);
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Confirm clone placement at cursor position
    pub(super) fn handle_clone_confirm(&mut self, screen_point: iced::Point) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let point = DrawingPoint::from_screen(
                    screen_point,
                    state,
                    bounds,
                    c.drawings.snap_enabled(),
                );
                c.drawings.update_clone_position(point);
                if let Some(id) = c.drawings.confirm_clone_placement() {
                    c.drawings.select(id);
                }
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                let state = c.state();
                let bounds = state.bounds.size();
                let point = DrawingPoint::from_screen(
                    screen_point,
                    state,
                    bounds,
                    c.drawings.snap_enabled(),
                );
                c.drawings.update_clone_position(point);
                if let Some(id) = c.drawings.confirm_clone_placement() {
                    c.drawings.select(id);
                }
                c.mut_state().cache.clear_drawings();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Cancel clone placement
    pub(super) fn handle_clone_cancel(&mut self) {
        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                c.drawings.cancel_clone_placement();
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.cancel_clone_placement();
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Rebuild the current chart by re-requesting data load
    pub(super) fn rebuild_current_chart(&mut self) -> Option<Effect> {
        let ticker_info = self.ticker_info?;
        let kind = self.content.kind();

        match kind {
            ContentKind::CandlestickChart | ContentKind::HeatmapChart => {}
            _ => return None,
        };

        let date_range = DateRange::last_n_days(1);

        // Reset content to show loading screen
        self.content = Content::new_for_kind(kind, ticker_info, &self.settings);
        self.chart_data = None;

        let days_total = date_range.num_days() as usize;
        self.loading_status = LoadingStatus::LoadingFromCache {
            schema: DataSchema::Trades,
            days_total,
            days_loaded: 0,
            items_loaded: 0,
        };

        let basis = self
            .settings
            .selected_basis
            .unwrap_or(ChartBasis::Time(Timeframe::M5));

        Some(Effect::LoadChart {
            config: ChartConfig {
                ticker: ticker_info.ticker,
                basis,
                date_range,
                chart_type: kind.to_chart_type(),
            },
            ticker_info,
        })
    }

    /// Center the chart view on the last price, showing ~50 bars
    pub(super) fn center_last_price(&mut self) {
        use crate::chart::Chart;
        use crate::chart::candlestick::domain_to_exchange_price;
        use crate::chart::scale::linear::PriceInfoLabel;

        // Get last candle close price from pane's chart_data
        let last_close = self
            .chart_data
            .as_ref()
            .and_then(|d| d.candles.last())
            .map(|c| domain_to_exchange_price(c.close));

        match &mut self.content {
            Content::Kline { chart: Some(c), .. } => {
                let chart = c.mut_state();
                let x_translation = 0.5 * (chart.bounds.width / chart.scaling)
                    - (8.0 * chart.cell_width / chart.scaling);
                chart.translation.x = x_translation;

                if let Some(price) = last_close {
                    let y = chart.price_to_y(price);
                    chart.translation.y = -y;
                }

                chart.cache.clear_all();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                let chart = c.mut_state();
                let x_translation = 0.5 * (chart.bounds.width / chart.scaling)
                    - (8.0 * chart.cell_width / chart.scaling);
                chart.translation.x = x_translation;

                // For heatmap use last_price from ViewState, else
                // fall back to last candle close
                let price = chart
                    .last_price
                    .map(|lp| match lp {
                        PriceInfoLabel::Up(p)
                        | PriceInfoLabel::Down(p)
                        | PriceInfoLabel::Neutral(p) => p,
                    })
                    .or(last_close);

                if let Some(price) = price {
                    let y = chart.price_to_y(price);
                    chart.translation.y = -y;
                }

                chart.cache.clear_all();
            }
            _ => {}
        }
    }
}
