mod candle;
mod render;
mod studies;

use crate::chart::{
    Chart, PlotLimits, ViewState,
    drawing::{ChartDrawingAccess, DrawingManager},
    scale::linear::PriceInfoLabel,
};
use crate::screen::dashboard::pane::config::CandleStyle;
use data::FuturesTickerInfo;
use data::util::count_decimals;
use data::{
    Autoscale, Candle, ChartBasis, ChartData, Price as DomainPrice, Side, Trade, ViewConfig,
};
use data::{Price, PriceStep};
use iced::Vector;
use iced::widget::canvas::Cache;
use study::{CandleRenderConfig, Study as _};

use std::cell::{Cell, RefCell};
use std::time::Instant;

impl Chart for KlineChart {
    // Boilerplate: state, mut_state, drawings, studies, panel_cache,
    // panel_labels_cache, panel_crosshair_cache.
    crate::chart_impl!(KlineChart);

    fn invalidate_crosshair(&mut self) {
        self.chart.cache.clear_crosshair();
        self.panel_crosshair_cache.clear();
        self.side_panel_crosshair_cache.clear();
    }

    fn invalidate_all(&mut self) {
        self.invalidate_inner(true);
    }

    fn invalidate_view(&mut self) {
        self.invalidate_inner(false);
    }

    fn interval_keys(&self) -> Option<Vec<u64>> {
        match &self.basis {
            ChartBasis::Time(_) => None,
            ChartBasis::Tick(_) => {
                // Return actual timestamps for tick-based charts (not indices)
                Some(self.chart_data.candles.iter().map(|c| c.time.0).collect())
            }
        }
    }

    fn autoscaled_coords(&self) -> Vector {
        let chart = self.state();
        let x_cells = self
            .candle_replace_config()
            .map(|c| c.autoscale_x_cells)
            .unwrap_or(8.0);
        let x_translation = 0.5 * (chart.bounds.width / chart.scaling)
            - (x_cells * chart.cell_width / chart.scaling);
        Vector::new(x_translation, chart.translation.y)
    }

    fn supports_fit_autoscaling(&self) -> bool {
        true
    }

    fn is_empty(&self) -> bool {
        self.chart_data.candles.is_empty()
    }

    fn plot_limits(&self) -> PlotLimits {
        let cr = self.candle_replace_config();
        PlotLimits {
            max_cell_width: cr.map(|c| c.max_cell_width).unwrap_or(100.0),
            min_cell_width: cr.map(|c| c.min_cell_width).unwrap_or(1.0),
            max_cell_height: if self.has_candle_replace() {
                100.0
            } else {
                200.0
            },
            min_cell_height: 0.1,
            default_cell_width: cr.map(|c| c.default_cell_width).unwrap_or(4.0),
        }
    }

    fn side_panel_cache(&self) -> Option<&Cache> {
        Some(&self.side_panel_cache)
    }

    fn side_panel_crosshair_cache(&self) -> Option<&Cache> {
        Some(&self.side_panel_crosshair_cache)
    }

    fn hit_test_study_overlay(&self, point: iced::Point) -> Option<usize> {
        self.study_overlay_rects
            .borrow()
            .iter()
            .find(|(_, rect)| rect.contains(point))
            .map(|(idx, _)| *idx)
    }

    fn hit_test_study_detail_button(&self, point: iced::Point) -> Option<usize> {
        self.study_detail_button_rects
            .borrow()
            .iter()
            .find(|(_, rect)| rect.contains(point))
            .map(|(idx, _)| *idx)
    }
}

/// Reason studies need recomputation.
#[derive(Debug, Clone, Copy)]
pub(crate) enum StudiesDirtyReason {
    /// Data replaced, basis changed, or params changed — full recompute.
    FullRecompute,
    /// Only new trades appended — use incremental path.
    #[allow(dead_code)] // planned: used by incremental trade append path
    NewTradesAppended,
}

pub struct KlineChart {
    chart: ViewState,
    chart_data: ChartData,
    basis: ChartBasis,
    ticker_info: FuturesTickerInfo,
    last_tick: Instant,
    /// Drawing manager for chart annotations
    drawings: DrawingManager,
    /// Candlestick visual style
    pub(crate) candle_style: CandleStyle,
    /// Overlay studies (Big Trades, etc.)
    studies: Vec<Box<dyn study::Study>>,
    /// Why studies need recomputation on next invalidate (None = clean)
    studies_dirty: Option<StudiesDirtyReason>,
    /// Last visible time range passed to studies (for detecting view changes)
    last_visible_range: Option<(u64, u64)>,
    /// Rendering cache for panel-placement studies
    panel_cache: Cache,
    /// Rendering cache for panel Y-axis labels
    panel_labels_cache: Cache,
    /// Rendering cache for panel crosshair overlay
    panel_crosshair_cache: Cache,
    /// Rendering cache for side panel content
    side_panel_cache: Cache,
    /// Rendering cache for side panel crosshair overlay
    side_panel_crosshair_cache: Cache,
    /// Whether to show the debug performance overlay
    pub(crate) show_debug_info: bool,
    /// Last draw() call instant — uses Cell for interior mutability during draw
    last_draw_instant: Cell<Instant>,
    /// Rolling frame time in ms — uses Cell for interior mutability during draw
    last_frame_time_ms: Cell<f32>,
    /// Hit-test rectangles for study overlay labels (populated during draw)
    pub(crate) study_overlay_rects: RefCell<Vec<(usize, iced::Rectangle)>>,
    /// Hit-test rectangles for study detail icon buttons (populated during draw)
    pub(crate) study_detail_button_rects: RefCell<Vec<(usize, iced::Rectangle)>>,
}

impl KlineChart {
    /// Create new KlineChart from ChartData
    pub fn from_chart_data(
        chart_data: ChartData,
        basis: ChartBasis,
        ticker_info: FuturesTickerInfo,
        layout: ViewConfig,
    ) -> Self {
        let step = PriceStep::from_f32(ticker_info.tick_size);

        let initial_candle_window = 60;
        let cell_height_ratio = 1.0;
        let default_cell_width = 4.0;
        let autoscale_x_cells = 8.0;

        let (_, _, cell_height) = compute_initial_price_scale(
            &chart_data.candles,
            initial_candle_window,
            ticker_info.tick_size,
            cell_height_ratio,
        );

        let base_price_y = chart_data
            .candles
            .first()
            .map(|c| domain_to_exchange_price(c.close))
            .unwrap_or(Price::from_f32(0.0));

        let latest_x = chart_data.candles.last().map(|c| c.time.0).unwrap_or(0);

        let mut chart = ViewState::new(
            basis,
            step,
            count_decimals(ticker_info.tick_size),
            ticker_info,
            ViewConfig {
                splits: layout.splits,
                autoscale: Some(Autoscale::FitAll),
                side_splits: layout.side_splits,
            },
            default_cell_width,
            cell_height,
        );
        chart.base_price_y = base_price_y;
        chart.latest_x = latest_x;

        let x_translation = 0.5 * (chart.bounds.width / chart.scaling)
            - (autoscale_x_cells * chart.cell_width / chart.scaling);
        chart.translation.x = x_translation;

        KlineChart {
            chart,
            chart_data,
            basis,
            ticker_info,
            last_tick: Instant::now(),
            drawings: DrawingManager::new(),
            candle_style: CandleStyle::default(),
            studies: Vec::new(),
            studies_dirty: None,
            last_visible_range: None,
            panel_cache: Cache::default(),
            panel_labels_cache: Cache::default(),
            panel_crosshair_cache: Cache::default(),
            side_panel_cache: Cache::default(),
            side_panel_crosshair_cache: Cache::default(),
            show_debug_info: false,
            last_draw_instant: Cell::new(Instant::now()),
            last_frame_time_ms: Cell::new(0.0),
            study_overlay_rects: RefCell::new(Vec::new()),
            study_detail_button_rects: RefCell::new(Vec::new()),
        }
    }

    /// Switch chart basis (time-based or tick-based)
    /// Re-aggregates trades to candles using the new basis
    pub fn switch_basis(&mut self, new_basis: ChartBasis, ticker_info: FuturesTickerInfo) {
        self.basis = new_basis;
        self.ticker_info = ticker_info;

        // Re-aggregate trades to candles with new basis
        let new_candles = match new_basis {
            ChartBasis::Time(timeframe) => {
                let tick_size = DomainPrice::from_f32(ticker_info.tick_size);
                data::aggregate_trades_to_candles(
                    &self.chart_data.trades,
                    timeframe.to_milliseconds(),
                    tick_size,
                )
                .unwrap_or_default()
            }
            ChartBasis::Tick(tick_count) => {
                let tick_size = DomainPrice::from_f32(ticker_info.tick_size);
                data::aggregate_trades_to_ticks(&self.chart_data.trades, tick_count, tick_size)
                    .unwrap_or_default()
            }
        };

        self.chart_data.candles = new_candles;

        // Recalculate price scales
        let step = PriceStep::from_f32(ticker_info.tick_size);
        let initial_window = self
            .candle_replace_config()
            .map(|c| c.initial_candle_window)
            .unwrap_or(60);
        let cell_height_ratio = self
            .candle_replace_config()
            .map(|c| c.cell_height_ratio)
            .unwrap_or(1.0);

        let (_, _, cell_height) = compute_initial_price_scale(
            &self.chart_data.candles,
            initial_window,
            ticker_info.tick_size,
            cell_height_ratio,
        );

        self.chart.cell_height = cell_height;
        self.chart.basis = new_basis;
        self.chart.tick_size = step;

        // Update latest_x
        self.chart.latest_x = self
            .chart_data
            .candles
            .last()
            .map(|c| c.time.0)
            .unwrap_or(0);

        self.studies_dirty = Some(StudiesDirtyReason::FullRecompute);
        self.invalidate();
    }

    pub fn basis(&self) -> ChartBasis {
        self.basis
    }

    pub fn tick_size(&self) -> f32 {
        self.chart.tick_size.to_f32_lossy()
    }

    pub fn candle_style(&self) -> &CandleStyle {
        &self.candle_style
    }

    pub fn set_candle_style(&mut self, style: CandleStyle) {
        self.candle_style = style;
        self.invalidate();
    }

    pub fn set_show_debug_info(&mut self, show: bool) {
        self.show_debug_info = show;
        self.chart.cache.clear_crosshair();
    }

    pub fn chart_layout(&self) -> ViewConfig {
        self.chart.layout()
    }

    // ── CandleReplace helpers ─────────────────────────────────────────

    /// Returns true if a CandleReplace study is active.
    pub fn has_candle_replace(&self) -> bool {
        self.studies
            .iter()
            .any(|s| s.placement() == study::StudyPlacement::CandleReplace)
    }

    /// Get the CandleRenderConfig from the active CandleReplace study.
    pub fn candle_replace_config(&self) -> Option<CandleRenderConfig> {
        self.studies
            .iter()
            .find(|s| s.placement() == study::StudyPlacement::CandleReplace)
            .and_then(|s| s.candle_render_config())
    }

    pub fn change_tick_size(&mut self, new_tick_size: f32) {
        let chart = self.mut_state();

        let step = PriceStep::from_f32(new_tick_size);

        chart.cell_height *= new_tick_size / chart.tick_size.to_f32_lossy();
        chart.tick_size = step;

        self.invalidate();
    }

    pub fn last_update(&self) -> Instant {
        self.last_tick
    }

    pub fn invalidate(&mut self) {
        self.invalidate_inner(true);
    }

    /// Core invalidation with optional study recomputation.
    ///
    /// When `recompute_studies` is false (view-only changes like zoom/pan),
    /// skips the expensive visible-range check, study recompute, and VBP
    /// drawing computation. This avoids per-scroll-event study recalculation.
    fn invalidate_inner(&mut self, recompute_studies: bool) {
        let x_cells = self
            .candle_replace_config()
            .map(|c| c.autoscale_x_cells)
            .unwrap_or(8.0);
        let chart = &mut self.chart;

        if let Some(autoscale) = chart.layout.autoscale {
            match autoscale {
                Autoscale::Disabled => {
                    // No autoscaling - do nothing
                }
                Autoscale::CenterLatest => {
                    let x_translation = 0.5 * (chart.bounds.width / chart.scaling)
                        - (x_cells * chart.cell_width / chart.scaling);
                    chart.translation.x = x_translation;

                    if let Some(last_candle) = self.chart_data.candles.last() {
                        let y_low = chart.price_to_y(domain_to_exchange_price(last_candle.low));
                        let y_high = chart.price_to_y(domain_to_exchange_price(last_candle.high));
                        let y_close = chart.price_to_y(domain_to_exchange_price(last_candle.close));

                        let mut target_y_translation = -(y_low + y_high) / 2.0;

                        if chart.bounds.height > f32::EPSILON && chart.scaling > f32::EPSILON {
                            let visible_half_height = (chart.bounds.height / chart.scaling) / 2.0;

                            let view_center_y_centered = -target_y_translation;

                            let visible_y_top = view_center_y_centered - visible_half_height;
                            let visible_y_bottom = view_center_y_centered + visible_half_height;

                            let padding = chart.cell_height;

                            if y_close < visible_y_top {
                                target_y_translation = -(y_close - padding + visible_half_height);
                            } else if y_close > visible_y_bottom {
                                target_y_translation = -(y_close + padding - visible_half_height);
                            }
                        }

                        chart.translation.y = target_y_translation;
                    }
                }
                Autoscale::FitAll => {
                    let visible_region = chart.visible_region(chart.bounds.size());
                    let (start_interval, end_interval) = chart.interval_range(&visible_region);

                    let visible_candles: Vec<&Candle> = match &self.basis {
                        ChartBasis::Time(_) => self
                            .chart_data
                            .candles
                            .iter()
                            .filter(|c| c.time.0 >= start_interval && c.time.0 <= end_interval)
                            .collect(),
                        ChartBasis::Tick(_) => {
                            let start_idx = start_interval as usize;
                            let end_idx = end_interval as usize;
                            self.chart_data
                                .candles
                                .iter()
                                .rev()
                                .enumerate()
                                .filter(|(idx, _)| *idx >= start_idx && *idx <= end_idx)
                                .map(|(_, c)| c)
                                .collect()
                        }
                    };

                    if !visible_candles.is_empty() {
                        let highest = visible_candles
                            .iter()
                            .map(|c| c.high.to_f32())
                            .fold(f32::MIN, f32::max);
                        let lowest = visible_candles
                            .iter()
                            .map(|c| c.low.to_f32())
                            .fold(f32::MAX, f32::min);

                        let padding = (highest - lowest) * 0.05;
                        let price_span = (highest - lowest) + (2.0 * padding);

                        if price_span > 0.0 && chart.bounds.height > f32::EPSILON {
                            let padded_highest = highest + padding;
                            let chart_height = chart.bounds.height;
                            let tick_size = chart.tick_size.to_f32_lossy();

                            if tick_size > 0.0 {
                                chart.cell_height = (chart_height * tick_size) / price_span;
                                chart.base_price_y = Price::from_f32(padded_highest);
                                chart.translation.y = -chart_height / 2.0;
                            }
                        }
                    }
                }
            }
        }

        chart.cache.clear_all();
        self.panel_cache.clear();
        self.panel_labels_cache.clear();
        self.panel_crosshair_cache.clear();
        self.side_panel_cache.clear();
        self.side_panel_crosshair_cache.clear();

        if recompute_studies {
            // Check if visible range changed (triggers VBP recompute)
            if chart.bounds.width > 0.0 {
                let region = chart.visible_region(chart.bounds.size());
                let (earliest, latest) = chart.interval_range(&region);
                let new_range = if earliest < latest {
                    Some((earliest, latest))
                } else {
                    None
                };
                if new_range != self.last_visible_range {
                    self.last_visible_range = new_range;
                    // Only mark dirty if studies depend on visible_range
                    let has_range_dependent = self.studies.iter().any(|s| s.needs_visible_range());
                    if has_range_dependent {
                        self.studies_dirty = Some(StudiesDirtyReason::FullRecompute);
                    }
                }
            }

            if let Some(reason) = self.studies_dirty.take() {
                self.recompute_studies(reason);
            }

            // Pre-compute hit rectangles for study overlay labels
            self.recompute_study_overlay_rects();

            // Compute any pending VBP drawing profiles
            self.compute_vbp_drawings();
        }

        self.last_tick = Instant::now();
    }

    /// Pre-compute hit rectangles for the study overlay labels.
    ///
    /// Called from `invalidate_inner` after studies are (re)computed.
    /// The rectangles depend only on the study list structure (which
    /// studies exist and how many text lines each produces), so they
    /// are stable between cursor moves.  `draw_study_overlay` reads
    /// these cached rects instead of recomputing on every frame.
    fn recompute_study_overlay_rects(&self) {
        let line_height = crate::chart::TEXT_SIZE + crate::style::tokens::spacing::XXS;
        let x = crate::style::tokens::spacing::MD;
        let y_start = crate::style::tokens::spacing::MD + crate::style::tokens::text::HEADING;
        let hit_width: f32 = 300.0;
        let pad: f32 = 2.0;
        let detail_icon_size: f32 = crate::style::tokens::text::TINY;
        let detail_icon_pad: f32 = crate::style::tokens::spacing::XS;

        let mut rects = self.study_overlay_rects.borrow_mut();
        let mut detail_rects = self.study_detail_button_rects.borrow_mut();
        rects.clear();
        detail_rects.clear();

        let mut y = y_start;
        for (index, study) in self.studies.iter().enumerate() {
            let output = study.output();
            let lines = render::study_line_count(output);
            if lines == 0 {
                continue;
            }
            let y_before = y;
            let rect_height = lines as f32 * line_height;
            y += rect_height;

            rects.push((
                index,
                iced::Rectangle {
                    x: x - pad,
                    y: y_before - pad,
                    width: hit_width + 2.0 * pad,
                    height: rect_height + 2.0 * pad,
                },
            ));

            if study.has_detail_modal() {
                let btn_x = x + hit_width + detail_icon_pad;
                let btn_y = y_before + (line_height - detail_icon_size) / 2.0;
                detail_rects.push((
                    index,
                    iced::Rectangle {
                        x: btn_x,
                        y: btn_y,
                        width: detail_icon_size + 2.0 * detail_icon_pad,
                        height: detail_icon_size + 2.0 * detail_icon_pad,
                    },
                ));
            }
        }
    }

    /// Get a reference to the drawing manager
    pub fn drawings(&self) -> &DrawingManager {
        &self.drawings
    }

    /// Get a mutable reference to the drawing manager (invalidates drawing cache)
    pub fn drawings_mut(&mut self) -> &mut DrawingManager {
        self.chart.cache.clear_drawings();
        &mut self.drawings
    }

    /// Compute any pending VBP drawings that need (re)computation.
    pub fn compute_vbp_drawings(&mut self) {
        let pending = self.drawings.drain_vbp_computations();
        if pending.is_empty() {
            return;
        }

        let tick_size = data::Price::from_f32(self.ticker_info.tick_size);

        for id in pending {
            // Auto-fit Y bounds to candle high/low in the time range
            self.drawings
                .fit_vbp_to_candle_range(id, &self.chart_data.candles);

            let time_range = {
                let Some(d) = self.drawings.get(id) else {
                    continue;
                };
                if d.points.len() < 2 {
                    continue;
                }
                let t1 = d.points[0].time;
                let t2 = d.points[1].time;
                (t1.min(t2), t1.max(t2))
            };

            if let Some(d) = self.drawings.get_mut(id)
                && let Some(ref mut vbp) = d.vbp_study
            {
                vbp.set_range(time_range.0, time_range.1);
                let input = study::StudyInput {
                    candles: &self.chart_data.candles,
                    trades: Some(&self.chart_data.trades),
                    basis: self.basis,
                    tick_size,
                    visible_range: None,
                };
                if let Err(e) = vbp.compute(&input) {
                    log::warn!("VBP compute failed for drawing: {}", e);
                }
            }
        }

        self.chart.cache.clear_drawings();
    }
}

impl ChartDrawingAccess for KlineChart {
    fn drawings(&self) -> &DrawingManager {
        &self.drawings
    }

    fn drawings_mut(&mut self) -> &mut DrawingManager {
        &mut self.drawings
    }

    fn view_state(&self) -> &ViewState {
        &self.chart
    }

    fn invalidate_drawings_cache(&mut self) {
        self.chart.cache.clear_drawings();
    }

    fn invalidate_crosshair_cache(&mut self) {
        self.chart.cache.clear_crosshair();
    }

    fn compute_pending_vbp(&mut self) {
        self.compute_vbp_drawings();
    }

    fn candle_open_at_time(&self, time: u64) -> Option<data::Price> {
        let idx = self.chart_data.candles.partition_point(|c| c.time.0 < time);
        self.chart_data
            .candles
            .get(idx)
            .or_else(|| {
                idx.checked_sub(1)
                    .and_then(|i| self.chart_data.candles.get(i))
            })
            .map(|c| domain_to_exchange_price(c.open))
    }
}

impl KlineChart {
    /// Pure candle aggregation — no coordinate updates, no cache clears,
    /// no study updates. Trade must already be pushed to `chart_data.trades`.
    fn aggregate_trade_into_candle(&mut self, trade: &Trade) {
        let (buy_vol, sell_vol) = match trade.side {
            Side::Buy | Side::Bid => (data::Volume(trade.quantity.0), data::Volume(0.0)),
            Side::Sell | Side::Ask => (data::Volume(0.0), data::Volume(trade.quantity.0)),
        };

        match self.basis {
            ChartBasis::Time(tf) => {
                let interval = tf.to_milliseconds();
                if interval == 0 {
                    return;
                }
                let bucket_time = (trade.time.to_millis() / interval) * interval;

                if let Some(last) = self.chart_data.candles.last_mut()
                    && last.time.0 == bucket_time
                {
                    last.high = last.high.max(trade.price);
                    last.low = last.low.min(trade.price);
                    last.close = trade.price;
                    last.buy_volume = data::Volume(last.buy_volume.0 + buy_vol.0);
                    last.sell_volume = data::Volume(last.sell_volume.0 + sell_vol.0);
                } else {
                    self.chart_data.candles.push(Candle {
                        time: data::Timestamp::from_millis(bucket_time),
                        open: trade.price,
                        high: trade.price,
                        low: trade.price,
                        close: trade.price,
                        buy_volume: buy_vol,
                        sell_volume: sell_vol,
                    });
                }
            }
            ChartBasis::Tick(count) => {
                let count = count as usize;
                if count == 0 {
                    return;
                }
                let num_candles = self.chart_data.candles.len();
                let num_trades = self.chart_data.trades.len();
                let completed = if num_candles > 0 { num_candles - 1 } else { 0 };
                let trades_in_current = num_trades.saturating_sub(completed * count);

                if let Some(last) = self.chart_data.candles.last_mut()
                    && trades_in_current <= count
                {
                    last.high = last.high.max(trade.price);
                    last.low = last.low.min(trade.price);
                    last.close = trade.price;
                    last.buy_volume = data::Volume(last.buy_volume.0 + buy_vol.0);
                    last.sell_volume = data::Volume(last.sell_volume.0 + sell_vol.0);
                } else {
                    self.chart_data.candles.push(Candle {
                        time: trade.time,
                        open: trade.price,
                        high: trade.price,
                        low: trade.price,
                        close: trade.price,
                        buy_volume: buy_vol,
                        sell_volume: sell_vol,
                    });
                }
            }
        }
    }

    /// Rebuild the chart from scratch with the given trades.
    ///
    /// Clears all existing trades and candles, then replays the
    /// trades through aggregation. Used during replay seek to
    /// ensure the chart exactly represents `[start, position]`.
    pub fn rebuild_from_trades(&mut self, trades: &[Trade]) {
        self.chart_data.trades.clear();
        self.chart_data.candles.clear();

        // Reset incremental study state for full recompute
        for s in &mut self.studies {
            s.reset();
        }

        for trade in trades {
            self.chart_data.trades.push(*trade);
            self.aggregate_trade_into_candle(trade);
        }

        if let Some(first) = self.chart_data.candles.first() {
            self.chart.base_price_y = domain_to_exchange_price(first.close);
        }
        self.chart.latest_x = self
            .chart_data
            .candles
            .last()
            .map(|c| c.time.0)
            .unwrap_or(0);

        self.studies_dirty = Some(StudiesDirtyReason::FullRecompute);
        self.invalidate();
    }

    /// Append a single trade (live data or replay).
    ///
    /// Detects whether the trade creates a new candle or updates the
    /// current one, and branches accordingly:
    /// - First candle: initializes coordinate system + full invalidate
    /// - New candle: compensates X translation + full invalidate
    /// - Same candle: incremental study update + cache clears only
    pub fn append_trade(&mut self, trade: &Trade) {
        self.chart_data.trades.push(*trade);

        let candle_count_before = self.chart_data.candles.len();
        let old_latest_x = self.chart.latest_x;

        self.aggregate_trade_into_candle(trade);

        let new_candle_created = self.chart_data.candles.len() > candle_count_before;
        let is_first_candle = candle_count_before == 0 && new_candle_created;

        // Only set base_price_y for the very first candle (empty chart).
        // Otherwise base_price_y is managed by invalidate() (FitAll) or
        // stays fixed (CenterLatest/Disabled).
        if is_first_candle {
            self.chart.base_price_y = Price::from_units(trade.price.units());
        }

        // Update latest_x to the last candle's timestamp
        self.chart.latest_x = self
            .chart_data
            .candles
            .last()
            .map(|c| c.time.0)
            .unwrap_or(0);

        // Update last price label (cosmetic — doesn't affect coordinates)
        self.chart.last_price = Some(PriceInfoLabel::Neutral(Price::from_units(
            trade.price.units(),
        )));

        if is_first_candle {
            // First candle: initialize coordinate system + full invalidate
            self.studies_dirty = Some(StudiesDirtyReason::FullRecompute);
            self.invalidate();
        } else if new_candle_created {
            // New candle: compensate X shift when autoscale is off.
            // CenterLatest/FitAll handle translation in invalidate().
            let has_auto_x = matches!(
                self.chart.layout.autoscale,
                Some(Autoscale::CenterLatest) | Some(Autoscale::FitAll)
            );
            if !has_auto_x && old_latest_x != 0 {
                // Only time-based charts use latest_x in interval_to_x;
                // tick-based charts index by position, not timestamp.
                if let ChartBasis::Time(_) = self.basis {
                    let shift = self.chart.interval_to_x(old_latest_x);
                    self.chart.translation.x -= shift;
                }
            }
            self.studies_dirty = Some(StudiesDirtyReason::FullRecompute);
            self.invalidate();
        } else {
            // Same candle updated: incremental study update + cache clears
            if !self.studies.is_empty() {
                let input = study::StudyInput {
                    candles: &self.chart_data.candles,
                    trades: Some(&self.chart_data.trades),
                    basis: self.basis,
                    tick_size: DomainPrice::from_f32(self.ticker_info.tick_size),
                    visible_range: None,
                };
                let trade_slice = std::slice::from_ref(trade);
                for s in &mut self.studies {
                    if let Err(e) = s.append_trades(trade_slice, &input) {
                        log::warn!("Study '{}' append error: {}", s.id(), e);
                    }
                }
            }
            self.chart.cache.clear_main();
            self.panel_cache.clear();
            self.panel_crosshair_cache.clear();
        }
    }
}

/// Convert domain price to exchange price
#[inline]
pub(crate) fn domain_to_exchange_price(price: DomainPrice) -> Price {
    Price::from_units(price.units())
}

/// Compute initial price scale from a slice of candles.
///
/// Returns `(price_high, price_low, cell_height)` based on the most recent
/// `window` candles. `cell_height_ratio` controls the vertical pixel density.
fn compute_initial_price_scale(
    candles: &[Candle],
    window: usize,
    tick_size: f32,
    cell_height_ratio: f32,
) -> (Price, Price, f32) {
    let step = PriceStep::from_f32(tick_size);

    let (scale_high, scale_low) = if !candles.is_empty() {
        let end_idx = candles.len();
        let start_idx = end_idx.saturating_sub(window);

        let recent_candles = &candles[start_idx..end_idx];
        let high = recent_candles
            .iter()
            .map(|c| domain_to_exchange_price(c.high))
            .max()
            .unwrap_or(Price::from_f32(0.0));
        let low = recent_candles
            .iter()
            .map(|c| domain_to_exchange_price(c.low))
            .min()
            .unwrap_or(Price::from_f32(0.0));
        (high, low)
    } else {
        (Price::from_f32(100.0), Price::from_f32(0.0))
    };

    let low_rounded = scale_low.round_to_side_step(true, step.into());
    let high_rounded = scale_high.round_to_side_step(false, step.into());

    let y_ticks = Price::steps_between_inclusive(low_rounded, high_rounded, step.into())
        .map(|n| n.saturating_sub(1))
        .unwrap_or(1)
        .max(1) as f32;

    let cell_height = (200.0 * cell_height_ratio) / y_ticks;

    (scale_high, scale_low, cell_height)
}
