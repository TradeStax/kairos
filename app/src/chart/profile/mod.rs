mod render;
mod studies;

use crate::chart::{
    Chart, PanelStudyInfo, PlotConstants, ViewState,
    drawing::{ChartDrawingAccess, DrawingManager},
};
use data::state::pane::{ProfileConfig, ProfilePeriod};
use data::util::count_decimals;
use data::{
    Autoscale, Candle, ChartBasis, ChartData, Price as DomainPrice, Side, Timeframe, Trade,
    ViewConfig,
};
use exchange::FuturesTickerInfo;
use exchange::util::{Price, PriceStep};
use iced::Vector;
use iced::widget::canvas::Cache;
use study::orderflow::profile_core;
use study::output::{NodeDetectionMethod, ProfileLevel, VolumeNode};

use std::time::Instant;

/// Standalone profile chart — entire pane shows a Volume-by-Price profile.
/// Y-axis = Price, horizontal bars = Volume.
pub struct ProfileChart {
    chart: ViewState,
    chart_data: ChartData,
    basis: ChartBasis,
    ticker_info: FuturesTickerInfo,
    last_tick: Instant,
    drawings: DrawingManager,

    // Computed profile state
    profile_levels: Vec<ProfileLevel>,
    quantum: i64,
    poc_index: Option<usize>,
    value_area: Option<(usize, usize)>,
    hvn_nodes: Vec<VolumeNode>,
    lvn_nodes: Vec<VolumeNode>,
    /// Fingerprint to skip redundant recomputes
    fingerprint: (usize, u64, u64, usize),

    // Display config
    display_config: ProfileConfig,

    // Overlay / panel studies
    studies: Vec<Box<dyn study::Study>>,
    studies_dirty: bool,
    last_visible_range: Option<(u64, u64)>,
    panel_cache: Cache,
    panel_labels_cache: Cache,
}

impl Chart for ProfileChart {
    fn state(&self) -> &ViewState {
        &self.chart
    }

    fn mut_state(&mut self) -> &mut ViewState {
        &mut self.chart
    }

    fn invalidate_crosshair(&mut self) {
        self.chart.cache.clear_crosshair();
    }

    fn invalidate_all(&mut self) {
        self.invalidate();
    }

    fn interval_keys(&self) -> Option<Vec<u64>> {
        // Profile chart has no time axis
        None
    }

    fn autoscaled_coords(&self) -> Vector {
        let chart = self.state();
        Vector::new(chart.translation.x, chart.translation.y)
    }

    fn supports_fit_autoscaling(&self) -> bool {
        true
    }

    fn is_empty(&self) -> bool {
        self.chart_data.candles.is_empty() && self.chart_data.trades.is_empty()
    }

    fn active_drawing_tool(&self) -> data::DrawingTool {
        self.drawings.active_tool()
    }

    fn has_pending_drawing(&self) -> bool {
        self.drawings.has_pending()
    }

    fn hit_test_drawing(
        &self,
        screen_point: iced::Point,
        bounds: iced::Size,
    ) -> Option<data::DrawingId> {
        use crate::chart::core::tokens;
        self.drawings.hit_test(
            screen_point,
            self.state(),
            bounds,
            tokens::drawing::HIT_TOLERANCE,
        )
    }

    fn hit_test_drawing_handle(
        &self,
        screen_point: iced::Point,
        bounds: iced::Size,
    ) -> Option<(data::DrawingId, usize)> {
        use crate::chart::core::tokens;
        self.drawings.hit_test_handle(
            screen_point,
            self.state(),
            bounds,
            tokens::drawing::HANDLE_SIZE,
        )
    }

    fn has_drawing_selection(&self) -> bool {
        !self.drawings.selected_ids().is_empty()
    }

    fn is_drawing_selected(&self, id: data::DrawingId) -> bool {
        self.drawings.is_selected(id)
    }

    fn is_drawing_locked(&self, id: data::DrawingId) -> bool {
        self.drawings.get(id).is_some_and(|d| d.locked)
    }

    fn has_clone_pending(&self) -> bool {
        self.drawings.has_clone_pending()
    }

    fn panel_studies(&self) -> Vec<PanelStudyInfo<'_>> {
        self.studies
            .iter()
            .filter(|s| s.placement() == study::StudyPlacement::Panel)
            .filter(|s| !matches!(s.output(), study::StudyOutput::Empty))
            .map(|s| PanelStudyInfo {
                name: s.name(),
                output: s.output(),
            })
            .collect()
    }

    fn panel_cache(&self) -> Option<&Cache> {
        Some(&self.panel_cache)
    }

    fn panel_labels_cache(&self) -> Option<&Cache> {
        Some(&self.panel_labels_cache)
    }
}

impl PlotConstants for ProfileChart {
    fn max_cell_width(&self) -> f32 {
        // X axis not meaningful for profile, keep fixed
        1.0
    }

    fn min_cell_width(&self) -> f32 {
        1.0
    }

    fn max_cell_height(&self) -> f32 {
        200.0
    }

    fn min_cell_height(&self) -> f32 {
        0.1
    }

    fn default_cell_width(&self) -> f32 {
        1.0
    }
}

impl ProfileChart {
    /// Create a new ProfileChart from loaded chart data.
    pub fn from_chart_data(
        chart_data: ChartData,
        ticker_info: FuturesTickerInfo,
        layout: ViewConfig,
        config: ProfileConfig,
    ) -> Self {
        let step = PriceStep::from_f32(ticker_info.tick_size);
        let basis = ChartBasis::Time(Timeframe::M5);

        // Compute initial cell_height from price range
        let (_, _, cell_height) = compute_initial_price_scale(
            &chart_data.candles,
            ticker_info.tick_size,
        );

        let base_price_y = chart_data
            .candles
            .iter()
            .map(|c| c.high)
            .max()
            .map(|p| Price::from_units(p.units()))
            .unwrap_or(Price::from_f32(0.0));

        // latest_x is not meaningful for profile, but ViewState needs it
        let latest_x = chart_data.candles.last().map(|c| c.time.0).unwrap_or(0);

        let mut chart = ViewState::new(
            basis,
            step,
            count_decimals(ticker_info.tick_size),
            ticker_info,
            ViewConfig {
                splits: layout.splits,
                autoscale: Some(Autoscale::FitAll),
            },
            1.0, // cell_width fixed at 1.0
            cell_height,
        );
        chart.base_price_y = base_price_y;
        chart.latest_x = latest_x;
        chart.translation.y = -chart.bounds.height / 2.0;

        let mut profile = ProfileChart {
            chart,
            chart_data,
            basis,
            ticker_info,
            last_tick: Instant::now(),
            drawings: DrawingManager::new(),
            profile_levels: Vec::new(),
            quantum: step.units.max(1),
            poc_index: None,
            value_area: None,
            hvn_nodes: Vec::new(),
            lvn_nodes: Vec::new(),
            fingerprint: (0, 0, 0, 0),
            display_config: config,
            studies: Vec::new(),
            studies_dirty: false,
            last_visible_range: None,
            panel_cache: Cache::default(),
            panel_labels_cache: Cache::default(),
        };
        profile.recompute_profile();
        profile
    }

    /// Apply a new display configuration.
    pub fn set_display_config(&mut self, config: ProfileConfig) {
        self.display_config = config;
        self.recompute_profile();
        self.invalidate();
    }

    pub fn display_config(&self) -> &ProfileConfig {
        &self.display_config
    }

    pub fn last_update(&self) -> Instant {
        self.last_tick
    }

    pub fn chart_layout(&self) -> ViewConfig {
        self.chart.layout()
    }

    pub fn drawings(&self) -> &DrawingManager {
        &self.drawings
    }

    pub fn drawings_mut(&mut self) -> &mut DrawingManager {
        self.chart.cache.clear_drawings();
        &mut self.drawings
    }

    // ── Profile computation ───────────────────────────────────────────

    /// Rebuild the volume profile from chart data.
    fn recompute_profile(&mut self) {
        let tick_size = DomainPrice::from_f32(self.ticker_info.tick_size);
        let tick_units = tick_size.units().max(1);

        let group_quantum = if self.display_config.auto_grouping {
            tick_units * self.display_config.auto_group_factor.max(1)
        } else {
            tick_units * self.display_config.manual_ticks.max(1)
        };

        // Compute fingerprint and profile levels from data slices
        // (scoped borrow to avoid conflict with self.fingerprint)
        let (new_fp, levels) = {
            let (candle_slice, trade_slice) = self.resolve_data_slice();

            let fp = (
                trade_slice.len(),
                trade_slice.first().map(|t| t.time.0).unwrap_or(0),
                trade_slice.last().map(|t| t.time.0).unwrap_or(0),
                candle_slice.len(),
            );

            if fp == self.fingerprint && !self.profile_levels.is_empty() {
                return;
            }

            let lvls = if !trade_slice.is_empty() {
                profile_core::build_profile_from_trades(
                    trade_slice,
                    tick_size,
                    group_quantum,
                )
            } else if !candle_slice.is_empty() {
                profile_core::build_profile_from_candles(
                    candle_slice,
                    tick_size,
                    group_quantum,
                )
            } else {
                Vec::new()
            };

            (fp, lvls)
        };

        self.fingerprint = new_fp;
        self.profile_levels = levels;
        self.quantum = group_quantum;

        // POC
        self.poc_index = profile_core::find_poc_index(&self.profile_levels);

        // Value Area
        self.value_area = self.poc_index.and_then(|poc| {
            profile_core::calculate_value_area(
                &self.profile_levels,
                poc,
                self.display_config.value_area_pct as f64,
            )
        });

        // Volume nodes
        if self.display_config.show_hvn || self.display_config.show_lvn {
            let (hvn, lvn) = profile_core::detect_volume_nodes(
                &self.profile_levels,
                NodeDetectionMethod::Relative,
                self.display_config.hvn_threshold,
                NodeDetectionMethod::Relative,
                self.display_config.lvn_threshold,
                0.1,
            );
            self.hvn_nodes = hvn;
            self.lvn_nodes = lvn;
        } else {
            self.hvn_nodes.clear();
            self.lvn_nodes.clear();
        }
    }

    /// Resolve the data slice based on period settings.
    fn resolve_data_slice(&self) -> (&[Candle], &[Trade]) {
        match self.display_config.period {
            ProfilePeriod::AllData => {
                (&self.chart_data.candles, &self.chart_data.trades)
            }
            ProfilePeriod::Length => {
                let cutoff_ms = self.compute_length_cutoff();
                let candle_start = self
                    .chart_data
                    .candles
                    .partition_point(|c| c.time.0 < cutoff_ms);
                let trade_start = self
                    .chart_data
                    .trades
                    .partition_point(|t| t.time.0 < cutoff_ms);
                (
                    &self.chart_data.candles[candle_start..],
                    &self.chart_data.trades[trade_start..],
                )
            }
            ProfilePeriod::Custom => {
                let start = self.display_config.custom_start as u64;
                let end = self.display_config.custom_end as u64;
                if start == 0 && end == 0 {
                    return (&self.chart_data.candles, &self.chart_data.trades);
                }
                let cs = self
                    .chart_data
                    .candles
                    .partition_point(|c| c.time.0 < start);
                let ce = self
                    .chart_data
                    .candles
                    .partition_point(|c| c.time.0 <= end);
                let ts = self
                    .chart_data
                    .trades
                    .partition_point(|t| t.time.0 < start);
                let te = self
                    .chart_data
                    .trades
                    .partition_point(|t| t.time.0 <= end);
                (
                    &self.chart_data.candles[cs..ce],
                    &self.chart_data.trades[ts..te],
                )
            }
        }
    }

    fn compute_length_cutoff(&self) -> u64 {
        use data::state::pane::ProfileLengthUnit;
        let latest_ms = self
            .chart_data
            .candles
            .last()
            .map(|c| c.time.0)
            .or_else(|| self.chart_data.trades.last().map(|t| t.time.0))
            .unwrap_or(0);

        match self.display_config.length_unit {
            ProfileLengthUnit::Days => {
                let ms = self.display_config.length_value as u64 * 24 * 60 * 60 * 1000;
                latest_ms.saturating_sub(ms)
            }
            ProfileLengthUnit::Minutes => {
                let ms = self.display_config.length_value as u64 * 60 * 1000;
                latest_ms.saturating_sub(ms)
            }
            ProfileLengthUnit::Contracts => {
                // Count backwards by trade volume
                let target = self.display_config.length_value as f64;
                let mut accum = 0.0;
                for trade in self.chart_data.trades.iter().rev() {
                    accum += trade.quantity.value();
                    if accum >= target {
                        return trade.time.0;
                    }
                }
                0
            }
        }
    }

    pub fn invalidate(&mut self) {
        let chart = &mut self.chart;

        // Fit-all autoscaling: fit price range to visible area
        if let Some(Autoscale::FitAll) = chart.layout.autoscale {
            if !self.profile_levels.is_empty() {
                let highest = self
                    .profile_levels
                    .last()
                    .map(|l| l.price as f32)
                    .unwrap_or(0.0);
                let lowest = self
                    .profile_levels
                    .first()
                    .map(|l| l.price as f32)
                    .unwrap_or(0.0);

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

        chart.cache.clear_all();
        self.panel_cache.clear();
        self.panel_labels_cache.clear();

        // Check if visible range changed (triggers study recompute)
        if chart.bounds.width > 0.0 {
            let region = chart.visible_region(chart.bounds.size());
            let (_, _) = chart.interval_range(&region);
            let price_range = chart.price_range(&region);
            let new_range = Some((price_range.1.units() as u64, price_range.0.units() as u64));
            if new_range != self.last_visible_range {
                self.last_visible_range = new_range;
                self.studies_dirty = true;
            }
        }

        if self.studies_dirty {
            self.recompute_studies();
            self.studies_dirty = false;
        }

        self.last_tick = Instant::now();
    }

    /// Rebuild the chart from scratch with the given trades.
    pub fn rebuild_from_trades(&mut self, trades: &[Trade]) {
        self.chart_data.trades.clear();
        self.chart_data.candles.clear();

        for s in &mut self.studies {
            s.reset();
        }

        for trade in trades {
            self.append_trade(trade);
        }

        self.fingerprint = (0, 0, 0, 0); // force recompute
        self.recompute_profile();
        self.studies_dirty = true;
        self.invalidate();
    }

    /// Append a single trade during replay.
    pub fn append_trade(&mut self, trade: &Trade) {
        self.chart_data.trades.push(*trade);

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
                    return;
                }
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
            ChartBasis::Tick(_) => {} // Profile doesn't use tick basis
        }
    }
}

impl ChartDrawingAccess for ProfileChart {
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
}

/// Compute initial price scale from profile data.
fn compute_initial_price_scale(
    candles: &[Candle],
    tick_size: f32,
) -> (Price, Price, f32) {
    let step = PriceStep::from_f32(tick_size);

    let (scale_high, scale_low) = if !candles.is_empty() {
        let high = candles
            .iter()
            .map(|c| Price::from_units(c.high.units()))
            .max()
            .unwrap_or(Price::from_f32(0.0));
        let low = candles
            .iter()
            .map(|c| Price::from_units(c.low.units()))
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

    let cell_height = 200.0 / y_ticks;

    (scale_high, scale_low, cell_height)
}
