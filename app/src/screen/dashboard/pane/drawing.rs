use super::{Action, Content, State};
use super::types::{AiContextBubble, AiContextSummary};
use crate::chart::ViewState;
use crate::chart::drawing::{
    ChartDrawingAccess, Drawing, DrawingManager, DrawingPoint, snap,
};
use data::{
    ChartBasis, ChartConfig, ContentKind, DataSchema, DateRange, DrawingId,
    DrawingTool, LoadingStatus, Timeframe,
};
use iced::Size;

/// Format a number with comma thousands separators.
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result
}

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
            // Check if this was an AiContext drawing before resetting tool
            let is_ai_context = chart
                .drawings()
                .get(id)
                .is_some_and(|d| d.tool == DrawingTool::AiContext);

            chart.drawings_mut().set_tool(DrawingTool::None);
            chart.drawings_mut().select(id);
            chart.compute_pending_vbp();
            chart.invalidate_all_drawing_caches();

            if is_ai_context {
                self.show_ai_context_bubble(id);
            }

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
        let mut point = screen_to_drawing_point(chart, constrained);

        // Snap VBP preview Y to candle open price
        if chart.drawings().active_tool() == DrawingTool::VolumeProfile {
            if let Some(open) = chart.candle_open_at_time(point.time) {
                point.price = open;
            }
        }

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
        self.ai_context_bubble = None;
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().delete_selected();
            chart.invalidate_all_drawing_caches();
        }
    }

    /// Handle drawing selection
    pub(super) fn handle_drawing_select(&mut self, id: DrawingId) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            let is_ai_context = chart
                .drawings()
                .get(id)
                .is_some_and(|d| d.tool == DrawingTool::AiContext);
            chart.drawings_mut().select(id);
            chart.invalidate_all_drawing_caches();

            if is_ai_context {
                self.show_ai_context_bubble(id);
            } else {
                self.ai_context_bubble = None;
            }
        }
    }

    /// Handle drawing deselection
    pub(super) fn handle_drawing_deselect(&mut self) {
        self.ai_context_bubble = None;
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().clear_selection();
            chart.invalidate_all_drawing_caches();
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
        chart.invalidate_all_drawing_caches();
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
        chart.invalidate_all_drawing_caches();
    }

    /// Handle drawing drag end
    pub(super) fn handle_drawing_drag_end(&mut self) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().end_drag();
            chart.compute_pending_vbp();
            chart.invalidate_all_drawing_caches();
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
        use crate::modals::drawing::properties::DrawingPropertiesModal;
        use crate::modals::pane::Modal;

        // AiContext: re-show the bubble instead of properties modal
        let is_ai_context = self
            .content
            .drawing_chart()
            .and_then(|c| c.drawings().get(id))
            .is_some_and(|d| d.tool == DrawingTool::AiContext);
        if is_ai_context {
            self.show_ai_context_bubble(id);
            return;
        }

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
        update: &crate::modals::drawing::properties::DrawingUpdate,
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
            chart.invalidate_all_drawing_caches();
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
        if self
            .ai_context_bubble
            .as_ref()
            .is_some_and(|b| b.drawing_id == id)
        {
            self.ai_context_bubble = None;
        }
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().delete(id);
            chart.invalidate_all_drawing_caches();
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
            chart.invalidate_all_drawing_caches();
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
    ) -> Option<Action> {
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

        Some(Action::LoadChart {
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
    pub(super) fn rebuild_current_chart(&mut self) -> Option<Action> {
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

        Some(Action::LoadChart {
            config: ChartConfig {
                ticker: ticker_info.ticker,
                basis,
                date_range,
                chart_type: kind.to_chart_type(),
            },
            ticker_info,
        })
    }

    /// Show the AI context bubble for a completed AiContext drawing.
    ///
    /// Extracts chart context from the drawing's time range and builds
    /// a summary for the floating input panel.
    pub(super) fn show_ai_context_bubble(&mut self, id: DrawingId) {
        let chart = match self.content.drawing_chart() {
            Some(c) => c,
            None => return,
        };
        let drawing = match chart.drawings().get(id) {
            Some(d) if d.tool == DrawingTool::AiContext => d,
            _ => return,
        };
        if drawing.points.len() < 2 {
            return;
        }

        let t1 = drawing.points[0].time;
        let t2 = drawing.points[1].time;
        let (time_start, time_end) = (t1.min(t2), t1.max(t2));

        let p1 = drawing.points[0].price.to_f64();
        let p2 = drawing.points[1].price.to_f64();
        let (price_lo, price_hi) = if p1 < p2 { (p1, p2) } else { (p2, p1) };

        // Ticker + timeframe
        let ticker = self
            .ticker_info
            .map(|t| t.ticker.as_str().to_string())
            .unwrap_or_else(|| "?".into());
        let timeframe = self
            .settings
            .selected_basis
            .map(|b| format!("{}", b))
            .unwrap_or_else(|| "?".into());

        // Tick decimals for price formatting
        let tick_decimals = self
            .ticker_info
            .map(|t| {
                let ts = t.tick_size;
                if ts <= 0.0 {
                    2
                } else {
                    (-(ts as f64).log10()).ceil() as usize
                }
            })
            .unwrap_or(2);

        // Filter candles in range
        let candles: Vec<_> = self
            .chart_data
            .as_ref()
            .map(|cd| {
                cd.candles
                    .iter()
                    .filter(|c| c.time.0 >= time_start && c.time.0 <= time_end)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if candles.is_empty() {
            // No candles — delete drawing and show toast
            if let Some(chart) = self.content.drawing_chart_mut() {
                chart.drawings_mut().delete(id);
                chart.invalidate_all_drawing_caches();
            }
            self.notifications.push(
                crate::components::display::toast::Toast::warn(
                    "No candles in selected range".to_string(),
                ),
            );
            return;
        }

        // Aggregate stats
        let candle_count = candles.len();
        let total_volume: u64 =
            candles.iter().map(|c| c.volume() as u64).sum();
        let net_delta: i64 = candles
            .iter()
            .map(|c| c.buy_volume.0 as i64 - c.sell_volume.0 as i64)
            .sum();

        // Format timestamps
        let fmt_ts = |ms: u64| -> String {
            chrono::DateTime::from_timestamp_millis(ms as i64)
                .map(|dt| dt.format("%m/%d %H:%M").to_string())
                .unwrap_or_else(|| "?".into())
        };

        // Pre-format OHLCV lines (cap at 50)
        let max_lines = 50;
        let candle_ohlcv_lines: Vec<String> = candles
            .iter()
            .take(max_lines)
            .map(|c| {
                let ts =
                    chrono::DateTime::from_timestamp_millis(c.time.0 as i64)
                        .map(|dt| dt.format("%H:%M").to_string())
                        .unwrap_or_else(|| "?".into());
                let delta =
                    c.buy_volume.0 as i64 - c.sell_volume.0 as i64;
                let sign = if delta >= 0 { "+" } else { "" };
                format!(
                    "{} O={:.prec$} H={:.prec$} L={:.prec$} C={:.prec$} \
                     V={:.0} D={sign}{}",
                    ts,
                    c.open.to_f64(),
                    c.high.to_f64(),
                    c.low.to_f64(),
                    c.close.to_f64(),
                    c.volume(),
                    delta,
                    prec = tick_decimals,
                )
            })
            .collect();

        let summary = AiContextSummary {
            ticker: ticker.clone(),
            timeframe: timeframe.clone(),
            time_start_fmt: fmt_ts(time_start),
            time_end_fmt: fmt_ts(time_end),
            price_high: format!("{:.prec$}", price_hi, prec = tick_decimals),
            price_low: format!("{:.prec$}", price_lo, prec = tick_decimals),
            candle_count,
            total_volume: format_number(total_volume),
            net_delta: {
                let sign = if net_delta >= 0 { "+" } else { "" };
                format!("{sign}{}", format_number(net_delta.unsigned_abs()))
            },
            candle_ohlcv_lines,
        };

        self.ai_context_bubble = Some(AiContextBubble {
            drawing_id: id,
            input_text: String::new(),
            range_summary: summary,
        });
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
            Content::Candlestick { chart: Some(c), .. } => {
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
