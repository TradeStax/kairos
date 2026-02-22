use super::{Content, Effect, State};
use crate::chart::ViewState;
use crate::chart::drawing::{
    ChartDrawingAccess, Drawing, DrawingManager, DrawingPoint, snap,
};
use data::{
    ChartBasis, ChartConfig, ContentKind, DataSchema, DateRange, DrawingId,
    DrawingTool, LoadingStatus, Timeframe,
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
    Some((pending.tool, pending.points[0].as_screen_point(state, bounds)))
}

/// Apply shift constraint for drawing creation, using the pending
/// drawing's first point as anchor.
fn constrain_for_creation(
    chart: &dyn ChartDrawingAccess,
    screen_point: iced::Point,
    shift_held: bool,
) -> iced::Point {
    if !shift_held {
        return screen_point;
    }
    let state = chart.view_state();
    let bounds = state.bounds.size();
    creation_anchor(chart.drawings(), state, bounds)
        .map(|(tool, anchor)| snap::constrain_creation(tool, anchor, screen_point))
        .unwrap_or(screen_point)
}

/// Convert a screen point to a DrawingPoint using the chart's current
/// view state and snap setting.
fn screen_to_drawing_point(
    chart: &dyn ChartDrawingAccess,
    screen_point: iced::Point,
) -> DrawingPoint {
    let state = chart.view_state();
    let bounds = state.bounds.size();
    let snap = chart.drawings().snap_enabled();
    DrawingPoint::from_screen(screen_point, state, bounds, snap)
}

impl State {
    /// Handle drawing click at a screen position.
    /// Returns `true` if a drawing was completed and the tool was
    /// auto-switched to selection mode.
    pub(super) fn handle_drawing_click(
        &mut self,
        screen_point: iced::Point,
        shift_held: bool,
    ) -> bool {
        let Some(chart) = self.content.drawing_chart_mut() else {
            return false;
        };

        let constrained = constrain_for_creation(chart, screen_point, shift_held);
        let mut point = screen_to_drawing_point(chart, constrained);

        // Snap first VBP point's Y to candle open price
        if chart.drawings().active_tool() == DrawingTool::VolumeProfile
            && !chart.drawings().has_pending()
        {
            if let Some(open) = chart.candle_open_at_time(point.time) {
                point.price = open;
            }
        }

        let completed_id = if chart.drawings().has_pending() {
            chart.drawings_mut().complete_drawing(point)
        } else {
            chart.drawings_mut().start_drawing(point)
        };

        if let Some(id) = completed_id {
            chart.drawings_mut().set_tool(DrawingTool::None);
            chart.drawings_mut().select(id);
            chart.compute_pending_vbp();
            chart.invalidate_drawings_cache();
            chart.invalidate_crosshair_cache();
            return true;
        }
        chart.invalidate_crosshair_cache();
        false
    }

    /// Handle drawing move (update preview)
    pub(super) fn handle_drawing_move(
        &mut self,
        screen_point: iced::Point,
        shift_held: bool,
    ) {
        let Some(chart) = self.content.drawing_chart_mut() else {
            return;
        };
        if !chart.drawings().has_pending() {
            return;
        }

        let constrained = constrain_for_creation(chart, screen_point, shift_held);
        let point = screen_to_drawing_point(chart, constrained);
        chart.drawings_mut().update_preview(point);
        chart.invalidate_crosshair_cache();
    }

    /// Handle drawing cancel (Escape key)
    pub(super) fn handle_drawing_cancel(&mut self) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().cancel_pending();
            chart.invalidate_crosshair_cache();
        }
    }

    /// Handle drawing delete (Delete/Backspace key)
    pub(super) fn handle_drawing_delete(&mut self) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().delete_selected();
            chart.invalidate_drawings_cache();
            chart.invalidate_crosshair_cache();
        }
    }

    /// Handle drawing selection
    pub(super) fn handle_drawing_select(&mut self, id: DrawingId) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().select(id);
            chart.invalidate_drawings_cache();
            chart.invalidate_crosshair_cache();
        }
    }

    /// Handle drawing deselection
    pub(super) fn handle_drawing_deselect(&mut self) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().clear_selection();
            chart.invalidate_drawings_cache();
            chart.invalidate_crosshair_cache();
        }
    }

    /// Handle whole-drawing drag (moving entire drawing)
    pub(super) fn handle_drawing_drag(
        &mut self,
        screen_point: iced::Point,
        shift_held: bool,
    ) {
        let Some(chart) = self.content.drawing_chart_mut() else {
            return;
        };

        // Axis-lock when shift held
        let constrained = if shift_held {
            chart
                .drawings()
                .drag_start_screen()
                .map(|start| snap::constrain_axis(start, screen_point))
                .unwrap_or(screen_point)
        } else {
            screen_point
        };

        let point = screen_to_drawing_point(chart, constrained);
        let selected: Vec<DrawingId> =
            chart.drawings().selected_ids().iter().copied().collect();

        if let Some(&id) = selected.first() {
            if chart.drawings().is_dragging() {
                chart.drawings_mut().update_drag(id, point);
            } else {
                chart.drawings_mut().start_drag(point, id, screen_point);
            }
        }
        chart.invalidate_drawings_cache();
        chart.invalidate_crosshair_cache();
    }

    /// Handle single-handle drag
    pub(super) fn handle_drawing_handle_drag(
        &mut self,
        screen_point: iced::Point,
        handle_index: usize,
        shift_held: bool,
    ) {
        let Some(chart) = self.content.drawing_chart_mut() else {
            return;
        };

        let selected: Vec<DrawingId> =
            chart.drawings().selected_ids().iter().copied().collect();
        let Some(&id) = selected.first() else { return };

        let is_vbp = chart
            .drawings()
            .get(id)
            .is_some_and(|d| d.tool == DrawingTool::VolumeProfile);

        // Apply shift constraint based on tool/handle (skip for VBP)
        let constrained = if shift_held && !is_vbp {
            let state = chart.view_state();
            let bounds = state.bounds.size();
            chart
                .drawings()
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

        // VBP handles always snap to candle boundaries
        let point = if is_vbp {
            let state = chart.view_state();
            let bounds = state.bounds.size();
            DrawingPoint::from_screen(constrained, state, bounds, true)
        } else {
            screen_to_drawing_point(chart, constrained)
        };

        if !chart.drawings().is_dragging() {
            chart.drawings_mut().start_drag(point, id, screen_point);
        }
        chart.drawings_mut().update_handle_drag(id, handle_index, point);
        chart.invalidate_drawings_cache();
        chart.invalidate_crosshair_cache();
    }

    /// Handle drawing drag end
    pub(super) fn handle_drawing_drag_end(&mut self) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().end_drag();
            chart.compute_pending_vbp();
            chart.invalidate_drawings_cache();
            chart.invalidate_crosshair_cache();
        }
    }

    /// Get the first selected drawing ID, if any
    pub(super) fn get_selected_drawing_id(&self) -> Option<DrawingId> {
        self.content
            .drawing_chart()
            .and_then(|c| c.drawings().selected_ids().iter().next().copied())
    }

    /// Get the locked state for a drawing by ID
    pub(super) fn get_drawing_locked(&self, id: DrawingId) -> bool {
        self.content
            .drawing_chart()
            .and_then(|c| c.drawings().get(id))
            .map_or(false, |d| d.locked)
    }

    /// Open the drawing properties modal for the given drawing
    pub(super) fn handle_open_drawing_properties(&mut self, id: DrawingId) {
        use crate::modals::drawing_properties::DrawingPropertiesModal;
        use crate::modals::pane::Modal;

        let ticker_info = self.ticker_info;
        let modal = self.content.drawing_chart().and_then(|c| {
            c.drawings().get(id).map(|d| {
                let snapshot = d.to_serializable();
                DrawingPropertiesModal::new(
                    d.id,
                    d.tool,
                    &d.style,
                    d.locked,
                    d.visible,
                    d.label.clone(),
                    snapshot,
                    ticker_info,
                )
            })
        });

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
        if let Some(chart) = self.content.drawing_chart_mut() {
            if let Some(d) = chart.drawings_mut().get_mut(id) {
                d.style = update.style.clone();
                d.locked = update.locked;
                d.visible = update.visible;
                d.label = update.label.clone();
                // Sync VBP config to embedded study
                if d.tool == DrawingTool::VolumeProfile {
                    if let (Some(cfg), Some(study)) =
                        (&d.style.vbp_config, &mut d.vbp_study)
                    {
                        study.import_config(&cfg.params);
                        // Force Custom period to preserve drawing anchors
                        if d.points.len() >= 2 {
                            let t1 = d.points[0].time;
                            let t2 = d.points[1].time;
                            study.set_range(t1.min(t2), t1.max(t2));
                        }
                    }
                    chart.drawings_mut().queue_vbp_compute(id);
                }
            }
            chart.compute_pending_vbp();
            chart.invalidate_drawings_cache();
            chart.invalidate_crosshair_cache();
        }
    }

    /// Record undo for a property change and sync the default style.
    pub(super) fn finalize_drawing_properties(
        &mut self,
        id: DrawingId,
        before_snapshot: data::SerializableDrawing,
    ) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart
                .drawings_mut()
                .record_property_change(id, before_snapshot);
            if let Some(style) =
                chart.drawings().get(id).map(|d| d.style.clone())
            {
                chart.drawings_mut().set_default_style(style);
            }
            chart.compute_pending_vbp();
        }
    }

    /// Delete a specific drawing by ID (from context menu)
    pub(super) fn handle_drawing_context_delete(&mut self, id: DrawingId) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().delete(id);
            chart.invalidate_drawings_cache();
            chart.invalidate_crosshair_cache();
        }
    }

    /// Toggle lock state of a drawing
    pub(super) fn handle_drawing_toggle_lock(&mut self, id: DrawingId) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            if let Some(d) = chart.drawings_mut().get_mut(id) {
                d.locked = !d.locked;
            }
            chart.invalidate_drawings_cache();
        }
    }

    /// Start clone placement mode for a drawing
    pub(super) fn handle_drawing_clone(&mut self, id: DrawingId) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            if let Some(cloned_drawing) =
                chart.drawings().get(id).map(|original| {
                    let mut cloned =
                        Drawing::from_serializable(&original.to_serializable());
                    cloned.id = DrawingId::new();
                    cloned
                })
            {
                chart.drawings_mut().start_clone_placement(cloned_drawing);
            }
            chart.invalidate_crosshair_cache();
        }
    }

    /// Update clone placement position as cursor moves
    pub(super) fn handle_clone_move(&mut self, screen_point: iced::Point) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            let point = screen_to_drawing_point(chart, screen_point);
            chart.drawings_mut().update_clone_position(point);
            chart.invalidate_crosshair_cache();
        }
    }

    /// Confirm clone placement at cursor position
    pub(super) fn handle_clone_confirm(
        &mut self,
        screen_point: iced::Point,
    ) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            let point = screen_to_drawing_point(chart, screen_point);
            chart.drawings_mut().update_clone_position(point);
            if let Some(id) = chart.drawings_mut().confirm_clone_placement() {
                chart.drawings_mut().select(id);
            }
            chart.invalidate_drawings_cache();
            chart.invalidate_crosshair_cache();
        }
    }

    /// Cancel clone placement
    pub(super) fn handle_clone_cancel(&mut self) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().cancel_clone_placement();
            chart.invalidate_crosshair_cache();
        }
    }

    /// Rebuild the current chart with a specific number of days
    pub(super) fn rebuild_chart_with_days(
        &mut self,
        days: i64,
    ) -> Option<Effect> {
        let ticker_info = self.ticker_info?;
        let kind = self.content.kind();

        match kind {
            ContentKind::CandlestickChart | ContentKind::HeatmapChart => {}
            _ => return None,
        };

        let date_range = DateRange::last_n_days(days.max(1));

        self.content =
            Content::new_for_kind(kind, ticker_info, &self.settings);
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

    /// Rebuild the current chart by re-requesting data load
    pub(super) fn rebuild_current_chart(&mut self) -> Option<Effect> {
        let ticker_info = self.ticker_info?;
        let kind = self.content.kind();

        match kind {
            ContentKind::CandlestickChart | ContentKind::HeatmapChart => {}
            _ => return None,
        };

        let date_range = self
            .loaded_date_range
            .unwrap_or_else(|| DateRange::last_n_days(1));

        // Reset content to show loading screen
        self.content =
            Content::new_for_kind(kind, ticker_info, &self.settings);
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
                let x_translation = 0.5
                    * (chart.bounds.width / chart.scaling)
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
                let x_translation = 0.5
                    * (chart.bounds.width / chart.scaling)
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
