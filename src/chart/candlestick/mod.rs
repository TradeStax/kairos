mod candle;
mod footprint;
mod render;

use crate::chart::indicator::kline::KlineIndicatorImpl;
use crate::chart::{Chart, Message, PlotConstants, ViewState, drawing::DrawingManager, indicator};
use crate::modal::pane::settings::study;
use data::util::count_decimals;
use data::{
    Autoscale, Candle, ChartBasis, ChartData, ClusterKind, ClusterScaling, FootprintStudy,
    KlineChartKind, KlineIndicator, Price as DomainPrice, Side, Trade, ViewConfig,
};
use exchange::FuturesTickerInfo;
use exchange::util::{Price, PriceStep};

use iced::{Element, Vector};

use enum_map::EnumMap;
use std::collections::BTreeMap;
use std::time::Instant;

impl Chart for KlineChart {
    type IndicatorKind = KlineIndicator;

    fn state(&self) -> &ViewState {
        &self.chart
    }

    fn mut_state(&mut self) -> &mut ViewState {
        &mut self.chart
    }

    fn invalidate_crosshair(&mut self) {
        self.chart.cache.clear_crosshair();
        self.indicators
            .values_mut()
            .filter_map(Option::as_mut)
            .for_each(|indi| indi.clear_crosshair_caches());
    }

    fn invalidate_all(&mut self) {
        self.invalidate();
    }

    fn view_indicators(&'_ self, enabled: &[Self::IndicatorKind]) -> Vec<Element<'_, Message>> {
        let chart_state = self.state();
        let visible_region = chart_state.visible_region(chart_state.bounds.size());
        let (earliest, latest) = chart_state.interval_range(&visible_region);
        if earliest > latest {
            return vec![];
        }

        let mut elements = vec![];

        for selected_indicator in enabled {
            // Overlay indicators are drawn on the main chart canvas,
            // not as separate panel elements.
            if selected_indicator.is_overlay() {
                continue;
            }
            if let Some(indi) = self.indicators[*selected_indicator].as_ref() {
                elements.push(indi.element(chart_state, earliest..=latest));
            }
        }
        elements
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
        let x_translation = match &self.kind {
            KlineChartKind::Footprint { .. } => {
                0.5 * (chart.bounds.width / chart.scaling) - (chart.cell_width / chart.scaling)
            }
            KlineChartKind::Candles => {
                0.5 * (chart.bounds.width / chart.scaling)
                    - (8.0 * chart.cell_width / chart.scaling)
            }
        };
        Vector::new(x_translation, chart.translation.y)
    }

    fn supports_fit_autoscaling(&self) -> bool {
        true
    }

    fn is_empty(&self) -> bool {
        self.chart_data.candles.is_empty()
    }

    fn active_drawing_tool(&self) -> data::DrawingTool {
        self.drawings.active_tool()
    }
}

impl PlotConstants for KlineChart {
    fn min_scaling(&self) -> f32 {
        match self.kind {
            KlineChartKind::Footprint { .. } => 0.05,
            KlineChartKind::Candles => 0.1,
        }
    }

    fn max_scaling(&self) -> f32 {
        match self.kind {
            KlineChartKind::Footprint { .. } => 2.0,
            KlineChartKind::Candles => 5.0,
        }
    }

    fn max_cell_width(&self) -> f32 {
        match self.kind {
            KlineChartKind::Footprint { .. } => 500.0,
            KlineChartKind::Candles => 100.0,
        }
    }

    fn min_cell_width(&self) -> f32 {
        match self.kind {
            KlineChartKind::Footprint { .. } => 10.0,
            KlineChartKind::Candles => 1.0,
        }
    }

    fn max_cell_height(&self) -> f32 {
        match self.kind {
            KlineChartKind::Footprint { .. } => 100.0,
            KlineChartKind::Candles => 200.0,
        }
    }

    fn min_cell_height(&self) -> f32 {
        match self.kind {
            KlineChartKind::Footprint { .. } => 1.0,
            KlineChartKind::Candles => 1.0,
        }
    }

    fn default_cell_width(&self) -> f32 {
        match self.kind {
            KlineChartKind::Footprint { .. } => 80.0,
            KlineChartKind::Candles => 4.0,
        }
    }
}

pub struct KlineChart {
    chart: ViewState,
    chart_data: ChartData,
    basis: ChartBasis,
    ticker_info: FuturesTickerInfo,
    indicators: EnumMap<KlineIndicator, Option<Box<dyn KlineIndicatorImpl>>>,
    pub(crate) kind: KlineChartKind,
    study_configurator: study::Configurator<FootprintStudy>,
    last_tick: Instant,
    /// Cached footprints per candle (index -> footprint)
    /// Invalidated on basis change or data update
    footprint_cache: Option<Vec<BTreeMap<Price, TradeGroup>>>,
    /// Cache revision for invalidation tracking
    cache_revision: u64,
    /// Drawing manager for chart annotations
    pub drawings: DrawingManager,
}

impl KlineChart {
    /// Create new KlineChart from ChartData
    pub fn from_chart_data(
        chart_data: ChartData,
        basis: ChartBasis,
        ticker_info: FuturesTickerInfo,
        layout: ViewConfig,
        enabled_indicators: &[KlineIndicator],
        kind: KlineChartKind,
    ) -> Self {
        let step = PriceStep::from_f32(ticker_info.tick_size);

        // Calculate price scale from candles
        let (scale_high, scale_low) = if !chart_data.candles.is_empty() {
            let candle_count = match kind {
                KlineChartKind::Footprint { .. } => 12,
                KlineChartKind::Candles => 60,
            };
            let end_idx = chart_data.candles.len();
            let start_idx = end_idx.saturating_sub(candle_count);

            let recent_candles = &chart_data.candles[start_idx..end_idx];
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

        let base_price_y = chart_data
            .candles
            .first()
            .map(|c| domain_to_exchange_price(c.close))
            .unwrap_or(Price::from_f32(0.0));

        let latest_x = chart_data.candles.last().map(|c| c.time.0).unwrap_or(0);

        let low_rounded = scale_low.round_to_side_step(true, step);
        let high_rounded = scale_high.round_to_side_step(false, step);

        let y_ticks = Price::steps_between_inclusive(low_rounded, high_rounded, step)
            .map(|n| n.saturating_sub(1))
            .unwrap_or(1)
            .max(1) as f32;

        let cell_width = match kind {
            KlineChartKind::Footprint { .. } => 80.0,
            KlineChartKind::Candles => 4.0,
        };
        let cell_height = match kind {
            KlineChartKind::Footprint { .. } => 800.0 / y_ticks,
            KlineChartKind::Candles => 200.0 / y_ticks,
        };

        let mut chart = ViewState::new(
            basis,
            step,
            count_decimals(ticker_info.tick_size),
            ticker_info,
            ViewConfig {
                splits: layout.splits,
                autoscale: Some(Autoscale::FitAll),
            },
            cell_width,
            cell_height,
        );
        chart.base_price_y = base_price_y;
        chart.latest_x = latest_x;

        let x_translation = match &kind {
            KlineChartKind::Footprint { .. } => {
                0.5 * (chart.bounds.width / chart.scaling) - (chart.cell_width / chart.scaling)
            }
            KlineChartKind::Candles => {
                0.5 * (chart.bounds.width / chart.scaling)
                    - (8.0 * chart.cell_width / chart.scaling)
            }
        };
        chart.translation.x = x_translation;

        // Initialize indicators
        let mut indicators = EnumMap::default();
        for &i in enabled_indicators {
            let mut indi = indicator::kline::make_empty(i);
            indi.rebuild_from_candles(&chart_data.candles, basis);
            indicators[i] = Some(indi);
        }

        KlineChart {
            chart,
            chart_data,
            basis,
            ticker_info,
            indicators,
            kind,
            study_configurator: study::Configurator::new(),
            last_tick: Instant::now(),
            footprint_cache: None, // Lazy initialization on first use
            cache_revision: 0,
            drawings: DrawingManager::new(),
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
        let (scale_high, scale_low) = if !self.chart_data.candles.is_empty() {
            let candle_count = match self.kind {
                KlineChartKind::Footprint { .. } => 12,
                KlineChartKind::Candles => 60,
            };
            let end_idx = self.chart_data.candles.len();
            let start_idx = end_idx.saturating_sub(candle_count);

            let recent_candles = &self.chart_data.candles[start_idx..end_idx];
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

        let low_rounded = scale_low.round_to_side_step(true, step);
        let high_rounded = scale_high.round_to_side_step(false, step);

        let y_ticks = Price::steps_between_inclusive(low_rounded, high_rounded, step)
            .map(|n| n.saturating_sub(1))
            .unwrap_or(1)
            .max(1) as f32;

        let cell_height = match self.kind {
            KlineChartKind::Footprint { .. } => 800.0 / y_ticks,
            KlineChartKind::Candles => 200.0 / y_ticks,
        };

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

        // Rebuild indicators
        self.indicators
            .values_mut()
            .filter_map(Option::as_mut)
            .for_each(|indi| indi.rebuild_from_candles(&self.chart_data.candles, new_basis));

        // Invalidate footprint cache
        self.invalidate_footprint_cache();

        self.invalidate();
    }

    pub fn kind(&self) -> &KlineChartKind {
        &self.kind
    }

    pub fn basis(&self) -> ChartBasis {
        self.basis
    }

    pub fn tick_size(&self) -> f32 {
        self.chart.tick_size.to_f32_lossy()
    }

    pub fn study_configurator(&self) -> &study::Configurator<FootprintStudy> {
        &self.study_configurator
    }

    pub fn update_study_configurator(&mut self, message: study::Message<FootprintStudy>) {
        let KlineChartKind::Footprint {
            ref mut studies, ..
        } = self.kind
        else {
            return;
        };

        match self.study_configurator.update(message) {
            Some(study::Action::ToggleStudy(study, is_selected)) => {
                if is_selected {
                    let already_exists = studies.iter().any(|s| s.is_same_type(&study));
                    if !already_exists {
                        studies.push(study);
                    }
                } else {
                    studies.retain(|s| !s.is_same_type(&study));
                }
            }
            Some(study::Action::ConfigureStudy(study)) => {
                if let Some(existing_study) = studies.iter_mut().find(|s| s.is_same_type(&study)) {
                    *existing_study = study;
                }
            }
            None => {}
        }

        self.invalidate();
    }

    pub fn chart_layout(&self) -> ViewConfig {
        self.chart.layout()
    }

    pub fn set_cluster_kind(&mut self, new_kind: ClusterKind) {
        if let KlineChartKind::Footprint {
            ref mut clusters, ..
        } = self.kind
        {
            *clusters = new_kind;
        }

        self.invalidate();
    }

    pub fn set_cluster_scaling(&mut self, new_scaling: ClusterScaling) {
        if let KlineChartKind::Footprint {
            ref mut scaling, ..
        } = self.kind
        {
            *scaling = new_scaling;
        }

        self.invalidate();
    }

    pub fn change_tick_size(&mut self, new_tick_size: f32) {
        let chart = self.mut_state();

        let step = PriceStep::from_f32(new_tick_size);

        chart.cell_height *= new_tick_size / chart.tick_size.to_f32_lossy();
        chart.tick_size = step;

        // No need to rebuild candles - they're independent of tick size
        // Just notify indicators
        self.indicators
            .values_mut()
            .filter_map(Option::as_mut)
            .for_each(|indi| indi.on_ticksize_change(&self.chart_data.candles, self.basis));

        // Invalidate footprint cache since tick size changed
        self.invalidate_footprint_cache();

        self.invalidate();
    }

    pub fn studies(&self) -> Option<Vec<FootprintStudy>> {
        match &self.kind {
            KlineChartKind::Footprint { studies, .. } => Some(studies.clone()),
            _ => None,
        }
    }

    pub fn set_studies(&mut self, new_studies: Vec<FootprintStudy>) {
        if let KlineChartKind::Footprint {
            ref mut studies, ..
        } = self.kind
        {
            *studies = new_studies;
        }

        self.invalidate();
    }

    fn calc_qty_scales(
        &self,
        earliest: u64,
        latest: u64,
        highest: Price,
        lowest: Price,
        step: PriceStep,
        cluster_kind: ClusterKind,
    ) -> f32 {
        let rounded_highest = highest.round_to_side_step(false, step).add_steps(1, step);
        let rounded_lowest = lowest.round_to_side_step(true, step).add_steps(-1, step);

        // Calculate max quantity from trades in visible candles
        match &self.basis {
            ChartBasis::Time(timeframe) => {
                let interval_ms = timeframe.to_milliseconds();
                // For time-based, find candles in timestamp range
                let visible_candles: Vec<&Candle> = self
                    .chart_data
                    .candles
                    .iter()
                    .filter(|c| c.time.0 >= earliest && c.time.0 <= latest)
                    .collect();

                self.max_qty_from_candles(
                    &visible_candles,
                    rounded_highest,
                    rounded_lowest,
                    cluster_kind,
                    interval_ms,
                )
            }
            ChartBasis::Tick(_tick_count) => {
                // For tick-based, use index range
                let earliest = earliest as usize;
                let latest = latest as usize;
                let len = self.chart_data.candles.len();

                let visible_candles: Vec<&Candle> = self
                    .chart_data
                    .candles
                    .iter()
                    .rev()
                    .enumerate()
                    .filter(|(idx, _)| *idx >= earliest && *idx <= latest && *idx < len)
                    .map(|(_, c)| c)
                    .collect();

                // For tick charts, we use tick count to estimate time range
                let interval_estimate = 1000; // 1 second default
                self.max_qty_from_candles(
                    &visible_candles,
                    rounded_highest,
                    rounded_lowest,
                    cluster_kind,
                    interval_estimate,
                )
            }
        }
    }

    fn max_qty_from_candles(
        &self,
        candles: &[&Candle],
        highest: Price,
        lowest: Price,
        cluster_kind: ClusterKind,
        interval_ms: u64,
    ) -> f32 {
        // Build footprint from trades for visible candles
        let mut max_qty = 0.0_f32;

        for candle in candles {
            // Get trades within this candle's time range using binary search
            let candle_start = candle.time.0;
            let candle_end = candle.time.0 + interval_ms;

            // Find start index using binary search
            let start_idx = self
                .chart_data
                .trades
                .binary_search_by_key(&candle_start, |t| t.time.0)
                .unwrap_or_else(|i| i);

            // Find end index using binary search on the remaining slice
            let end_idx = self.chart_data.trades[start_idx..]
                .binary_search_by_key(&candle_end, |t| t.time.0)
                .map(|i| start_idx + i)
                .unwrap_or_else(|i| start_idx + i);

            // Get slice directly - O(1) instead of O(N)
            let trades_in_candle = &self.chart_data.trades[start_idx..end_idx];

            let footprint = self.build_footprint(trades_in_candle, highest, lowest);

            match cluster_kind {
                ClusterKind::BidAsk => {
                    for group in footprint.values() {
                        max_qty = max_qty.max(group.buy_qty.max(group.sell_qty));
                    }
                }
                ClusterKind::DeltaProfile => {
                    for group in footprint.values() {
                        max_qty = max_qty.max((group.buy_qty - group.sell_qty).abs());
                    }
                }
                ClusterKind::VolumeProfile => {
                    for group in footprint.values() {
                        max_qty = max_qty.max(group.buy_qty + group.sell_qty);
                    }
                }
                ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => {
                    // For simple cluster types, use buy_qty + sell_qty
                    for group in footprint.values() {
                        max_qty = max_qty.max(group.buy_qty + group.sell_qty);
                    }
                }
            }
        }

        max_qty
    }

    /// Build footprint for a single candle from trades
    fn build_footprint(
        &self,
        trades: &[Trade],
        highest: Price,
        lowest: Price,
    ) -> BTreeMap<Price, TradeGroup> {
        let step = self.chart.tick_size;
        let mut trades_map = BTreeMap::new();

        for trade in trades {
            let price_rounded = domain_to_exchange_price(trade.price).round_to_step(step);

            // Only include trades within visible price range
            if price_rounded < lowest || price_rounded > highest {
                continue;
            }

            let entry = trades_map.entry(price_rounded).or_insert(TradeGroup {
                buy_qty: 0.0,
                sell_qty: 0.0,
            });

            match trade.side {
                Side::Buy | Side::Bid => entry.buy_qty += trade.quantity.0 as f32,
                Side::Sell | Side::Ask => entry.sell_qty += trade.quantity.0 as f32,
            }
        }

        trades_map
    }

    /// Get or build cached footprint for a candle index
    ///
    /// Uses lazy initialization - builds full cache on first access,
    /// then serves from cache on subsequent calls.
    fn get_cached_footprint(
        &mut self,
        candle_index: usize,
        interval_ms: u64,
        highest: Price,
        lowest: Price,
    ) -> Option<BTreeMap<Price, TradeGroup>> {
        // Check if we need to rebuild cache
        if self.footprint_cache.is_none() {
            self.build_footprint_cache(interval_ms);
        }

        // Retrieve from cache
        self.footprint_cache
            .as_ref()
            .and_then(|cache| cache.get(candle_index).cloned())
            .or_else(|| {
                // Fallback: build on-demand if cache miss
                if candle_index < self.chart_data.candles.len() {
                    let candle = &self.chart_data.candles[candle_index];
                    let candle_start = candle.time.0;
                    let candle_end = candle.time.0 + interval_ms;

                    let start_idx = self
                        .chart_data
                        .trades
                        .binary_search_by_key(&candle_start, |t| t.time.0)
                        .unwrap_or_else(|i| i);

                    let end_idx = self.chart_data.trades[start_idx..]
                        .binary_search_by_key(&candle_end, |t| t.time.0)
                        .map(|i| start_idx + i)
                        .unwrap_or_else(|i| start_idx + i);

                    let trades_in_candle = &self.chart_data.trades[start_idx..end_idx];
                    Some(self.build_footprint(trades_in_candle, highest, lowest))
                } else {
                    None
                }
            })
    }

    /// Build complete footprint cache for all candles
    ///
    /// Pre-computes footprints for better rendering performance
    fn build_footprint_cache(&mut self, interval_ms: u64) {
        let candle_count = self.chart_data.candles.len();
        let mut cache = Vec::with_capacity(candle_count);

        // Use max price range to capture all trades
        let highest = Price::from_f32(f32::MAX);
        let lowest = Price::from_f32(0.0);

        for (idx, candle) in self.chart_data.candles.iter().enumerate() {
            let candle_start = candle.time.0;
            let candle_end = if idx + 1 < candle_count {
                self.chart_data.candles[idx + 1].time.0
            } else {
                candle.time.0 + interval_ms
            };

            // Binary search for trade range
            let start_idx = self
                .chart_data
                .trades
                .binary_search_by_key(&candle_start, |t| t.time.0)
                .unwrap_or_else(|i| i);

            let end_idx = self.chart_data.trades[start_idx..]
                .binary_search_by_key(&candle_end, |t| t.time.0)
                .map(|i| start_idx + i)
                .unwrap_or_else(|i| start_idx + i);

            let trades_in_candle = &self.chart_data.trades[start_idx..end_idx];
            let footprint = self.build_footprint(trades_in_candle, highest, lowest);
            cache.push(footprint);
        }

        self.footprint_cache = Some(cache);
        self.cache_revision += 1;
    }

    /// Invalidate footprint cache (call when data or basis changes)
    fn invalidate_footprint_cache(&mut self) {
        self.footprint_cache = None;
        self.cache_revision += 1;
    }

    pub fn last_update(&self) -> Instant {
        self.last_tick
    }

    pub fn invalidate(&mut self) {
        // Rebuild footprint cache for footprint charts if data has changed
        if matches!(self.kind, KlineChartKind::Footprint { .. }) && self.footprint_cache.is_none() {
            let interval_ms = match &self.basis {
                ChartBasis::Time(tf) => tf.to_milliseconds(),
                ChartBasis::Tick(_) => 1000, // Default estimate
            };
            self.build_footprint_cache(interval_ms);
        }

        let chart = &mut self.chart;

        if let Some(autoscale) = chart.layout.autoscale {
            match autoscale {
                Autoscale::Disabled => {
                    // No autoscaling - do nothing
                }
                Autoscale::CenterLatest => {
                    let x_translation = match &self.kind {
                        KlineChartKind::Footprint { .. } => {
                            0.5 * (chart.bounds.width / chart.scaling)
                                - (chart.cell_width / chart.scaling)
                        }
                        KlineChartKind::Candles => {
                            0.5 * (chart.bounds.width / chart.scaling)
                                - (8.0 * chart.cell_width / chart.scaling)
                        }
                    };
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
        for indi in self.indicators.values_mut().filter_map(Option::as_mut) {
            indi.clear_all_caches();
        }

        self.last_tick = Instant::now();
    }

    pub fn toggle_indicator(&mut self, indicator: KlineIndicator) {
        let panel_count =
            |indicators: &EnumMap<KlineIndicator, Option<Box<dyn KlineIndicatorImpl>>>| {
                indicators
                    .iter()
                    .filter(|(k, v)| v.is_some() && !k.is_overlay())
                    .count()
            };

        let prev_panel_count = panel_count(&self.indicators);

        if self.indicators[indicator].is_some() {
            self.indicators[indicator] = None;
        } else {
            let mut box_indi = indicator::kline::make_empty(indicator);
            box_indi.rebuild_from_candles(&self.chart_data.candles, self.basis);
            self.indicators[indicator] = Some(box_indi);
        }

        // Only adjust layout splits for panel (non-overlay) indicators.
        if !indicator.is_overlay()
            && let Some(main_split) = self.chart.layout.splits.first()
        {
            let current_panel_count = panel_count(&self.indicators);
            self.chart.layout.splits = data::util::calc_panel_splits(
                *main_split,
                current_panel_count,
                Some(prev_panel_count),
            );
        }

        self.invalidate();
    }
}

// ============================================================================
// HELPER TYPES
// ============================================================================

pub(crate) use crate::chart::study::TradeGroup;

/// Convert domain price to exchange price
#[inline]
pub(crate) fn domain_to_exchange_price(price: DomainPrice) -> Price {
    Price::from_units(price.units())
}
