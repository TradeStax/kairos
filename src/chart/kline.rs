use super::{
    Chart, Interaction, Message, PlotConstants, TEXT_SIZE, ViewState,
    indicator,
};
use crate::chart::indicator::kline::KlineIndicatorImpl;
use crate::{modal::pane::settings::study, style};
use data::{
    Autoscale, Candle, ChartBasis, ChartData, ClusterKind, ClusterScaling, FootprintStudy,
    KlineChartKind, KlineIndicator, Price as DomainPrice,
    Side, Trade, ViewConfig,
};
use data::util::{abbr_large_numbers, count_decimals};
use exchange::FuturesTickerInfo;
use exchange::util::{Price, PriceStep};

use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Event, Geometry, Path, Stroke};
use iced::{Alignment, Element, Point, Rectangle, Renderer, Size, Theme, Vector, mouse};

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
            if let Some(indi) = self.indicators[*selected_indicator].as_ref() {
                elements.push(indi.element(chart_state, earliest..=latest));
            }
        }
        elements
    }

    fn visible_timerange(&self) -> Option<(u64, u64)> {
        let chart = self.state();
        let region = chart.visible_region(chart.bounds.size());

        if region.width == 0.0 {
            return None;
        }

        match &self.basis {
            ChartBasis::Time(timeframe) => {
                let interval = timeframe.to_milliseconds();

                let (earliest, latest) = (
                    chart.x_to_interval(region.x) - (interval / 2),
                    chart.x_to_interval(region.x + region.width) + (interval / 2),
                );

                Some((earliest, latest))
            }
            ChartBasis::Tick(_) => {
                // For tick-based, return the index range
                let earliest = chart.x_to_interval(region.x + region.width);
                let latest = chart.x_to_interval(region.x);
                Some((earliest, latest))
            }
        }
    }

    fn interval_keys(&self) -> Option<Vec<u64>> {
        match &self.basis {
            ChartBasis::Time(_) => None,
            ChartBasis::Tick(_) => {
                // Return indices for tick-based charts
                Some((0..self.chart_data.candles.len() as u64).collect())
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
}

impl PlotConstants for KlineChart {
    fn min_scaling(&self) -> f32 {
        self.kind.min_scaling()
    }

    fn max_scaling(&self) -> f32 {
        self.kind.max_scaling()
    }

    fn max_cell_width(&self) -> f32 {
        self.kind.max_cell_width()
    }

    fn min_cell_width(&self) -> f32 {
        self.kind.min_cell_width()
    }

    fn max_cell_height(&self) -> f32 {
        self.kind.max_cell_height()
    }

    fn min_cell_height(&self) -> f32 {
        self.kind.min_cell_height()
    }

    fn default_cell_width(&self) -> f32 {
        self.kind.default_cell_width()
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
            let high = recent_candles.iter().map(|c| domain_to_exchange_price(c.high)).max().unwrap_or(Price::from_f32(0.0));
            let low = recent_candles.iter().map(|c| domain_to_exchange_price(c.low)).min().unwrap_or(Price::from_f32(0.0));
            (high, low)
        } else {
            (Price::from_f32(100.0), Price::from_f32(0.0))
        };

        let base_price_y = chart_data.candles.first()
            .map(|c| domain_to_exchange_price(c.close))
            .unwrap_or(Price::from_f32(0.0));

        let latest_x = chart_data.candles.last()
            .map(|c| c.time.0)
            .unwrap_or(0);

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
            indi.rebuild_from_candles(&chart_data.candles);
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
                data::aggregate_trades_to_candles(&self.chart_data.trades, timeframe.to_milliseconds(), tick_size)
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
            let high = recent_candles.iter().map(|c| domain_to_exchange_price(c.high)).max().unwrap_or(Price::from_f32(0.0));
            let low = recent_candles.iter().map(|c| domain_to_exchange_price(c.low)).min().unwrap_or(Price::from_f32(0.0));
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
        self.chart.latest_x = self.chart_data.candles.last()
            .map(|c| c.time.0)
            .unwrap_or(0);

        // Rebuild indicators
        self.indicators
            .values_mut()
            .filter_map(Option::as_mut)
            .for_each(|indi| indi.rebuild_from_candles(&self.chart_data.candles));

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
            .for_each(|indi| indi.on_ticksize_change(&self.chart_data.candles));

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
                let visible_candles: Vec<&Candle> = self.chart_data.candles
                    .iter()
                    .filter(|c| c.time.0 >= earliest && c.time.0 <= latest)
                    .collect();

                self.max_qty_from_candles(&visible_candles, rounded_highest, rounded_lowest, cluster_kind, interval_ms)
            }
            ChartBasis::Tick(_tick_count) => {
                // For tick-based, use index range
                let earliest = earliest as usize;
                let latest = latest as usize;
                let len = self.chart_data.candles.len();

                let visible_candles: Vec<&Candle> = self.chart_data.candles
                    .iter()
                    .rev()
                    .enumerate()
                    .filter(|(idx, _)| *idx >= earliest && *idx <= latest && *idx < len)
                    .map(|(_, c)| c)
                    .collect();

                // For tick charts, we use tick count to estimate time range
                let interval_estimate = 1000; // 1 second default
                self.max_qty_from_candles(&visible_candles, rounded_highest, rounded_lowest, cluster_kind, interval_estimate)
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
            let start_idx = self.chart_data.trades
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

    fn build_footprint(&self, trades: &[Trade], highest: Price, lowest: Price) -> BTreeMap<Price, TradeGroup> {
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

    pub fn last_update(&self) -> Instant {
        self.last_tick
    }

    pub fn invalidate(&mut self) {
        let chart = &mut self.chart;

        if let Some(autoscale) = chart.layout.autoscale {
            match autoscale {
                super::Autoscale::Disabled => {
                    // No autoscaling - do nothing
                }
                super::Autoscale::CenterLatest => {
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
                super::Autoscale::FitAll => {
                    let visible_region = chart.visible_region(chart.bounds.size());
                    let (start_interval, end_interval) = chart.interval_range(&visible_region);

                    let visible_candles: Vec<&Candle> = match &self.basis {
                        ChartBasis::Time(_) => {
                            self.chart_data.candles
                                .iter()
                                .filter(|c| c.time.0 >= start_interval && c.time.0 <= end_interval)
                                .collect()
                        }
                        ChartBasis::Tick(_) => {
                            let start_idx = start_interval as usize;
                            let end_idx = end_interval as usize;
                            self.chart_data.candles
                                .iter()
                                .rev()
                                .enumerate()
                                .filter(|(idx, _)| *idx >= start_idx && *idx <= end_idx)
                                .map(|(_, c)| c)
                                .collect()
                        }
                    };

                    if !visible_candles.is_empty() {
                        let highest = visible_candles.iter().map(|c| c.high.to_f32()).fold(f32::MIN, f32::max);
                        let lowest = visible_candles.iter().map(|c| c.low.to_f32()).fold(f32::MAX, f32::min);

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
        let prev_indi_count = self.indicators.values().filter(|v| v.is_some()).count();

        if self.indicators[indicator].is_some() {
            self.indicators[indicator] = None;
        } else {
            let mut box_indi = indicator::kline::make_empty(indicator);
            box_indi.rebuild_from_candles(&self.chart_data.candles);
            self.indicators[indicator] = Some(box_indi);
        }

        if let Some(main_split) = self.chart.layout.splits.first() {
            let current_indi_count = self.indicators.values().filter(|v| v.is_some()).count();
            self.chart.layout.splits = data::util::calc_panel_splits(
                *main_split,
                current_indi_count,
                Some(prev_indi_count),
            );
        }
    }
}

impl canvas::Program<Message> for KlineChart {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        super::canvas_interaction(self, interaction, event, bounds, cursor)
    }

    fn draw(
        &self,
        interaction: &Interaction,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let chart = self.state();

        if chart.bounds.width == 0.0 {
            return vec![];
        }

        let bounds_size = bounds.size();
        let palette = theme.extended_palette();

        let klines = chart.cache.main.draw(renderer, bounds_size, |frame| {
            let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

            frame.translate(center);
            frame.scale(chart.scaling);
            frame.translate(chart.translation);

            let region = chart.visible_region(frame.size());
            let (earliest, latest) = chart.interval_range(&region);

            let price_to_y = |price: Price| chart.price_to_y(price);
            let interval_to_x = |interval| chart.interval_to_x(interval);

            match &self.kind {
                KlineChartKind::Footprint {
                    clusters,
                    scaling,
                    studies,
                } => {
                    let (highest, lowest) = chart.price_range(&region);

                    let max_cluster_qty = self.calc_qty_scales(
                        earliest,
                        latest,
                        highest,
                        lowest,
                        chart.tick_size,
                        *clusters,
                    );

                    let cell_height_unscaled = chart.cell_height * chart.scaling;
                    let cell_width_unscaled = chart.cell_width * chart.scaling;

                    let text_size = {
                        let text_size_from_height = cell_height_unscaled.round().min(16.0) - 3.0;
                        let text_size_from_width =
                            (cell_width_unscaled * 0.1).round().min(16.0) - 3.0;

                        text_size_from_height.min(text_size_from_width)
                    };

                    let candle_width = 0.1 * chart.cell_width;
                    let content_spacing = ContentGaps::from_view(candle_width, chart.scaling);

                    let imbalance = studies.iter().find_map(|study| {
                        if let FootprintStudy::Imbalance {
                            threshold,
                            color_scale,
                            ignore_zeros,
                        } = study
                        {
                            Some((*threshold as usize, if *color_scale { Some(0) } else { None }, *ignore_zeros))
                        } else {
                            None
                        }
                    });

                    let show_text = {
                        let min_w = match clusters {
                            ClusterKind::VolumeProfile | ClusterKind::DeltaProfile => 80.0,
                            ClusterKind::BidAsk => 120.0,
                            ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => 120.0,
                        };
                        should_show_text(cell_height_unscaled, cell_width_unscaled, min_w)
                    };

                    // Draw nPOCs first (if study is enabled)
                    if let Some(lookback) = studies.iter().find_map(|study| {
                        if let FootprintStudy::NPoC { lookback } = study {
                            Some(*lookback)
                        } else {
                            None
                        }
                    }) {
                        draw_all_npocs(
                            &self.chart_data.candles,
                            &self.chart_data.trades,
                            &self.basis,
                            frame,
                            &price_to_y,
                            &interval_to_x,
                            candle_width,
                            chart.cell_width,
                            chart.cell_height,
                            chart.tick_size,
                            palette,
                            lookback,
                            earliest,
                            latest,
                            *clusters,
                            content_spacing,
                            imbalance.is_some(),
                        );
                    }

                    // Draw candles and footprint
                    let interval_ms = match &self.basis {
                        ChartBasis::Time(tf) => tf.to_milliseconds(),
                        ChartBasis::Tick(_) => 1000, // default estimate
                    };

                    render_candles(
                        &self.chart_data.candles,
                        &self.chart_data.trades,
                        &self.basis,
                        chart.tick_size,
                        interval_ms,
                        frame,
                        earliest,
                        latest,
                        interval_to_x,
                        |frame, x_position, candle, trades| {
                            let footprint = self.build_footprint(trades, highest, lowest);

                            let cluster_scaling =
                                effective_cluster_qty(*scaling, max_cluster_qty, &footprint, *clusters);

                            draw_clusters(
                                frame,
                                price_to_y,
                                x_position,
                                chart.cell_width,
                                chart.cell_height,
                                candle_width,
                                cluster_scaling,
                                palette,
                                text_size,
                                self.tick_size(),
                                show_text,
                                imbalance,
                                candle,
                                &footprint,
                                *clusters,
                                content_spacing,
                            );
                        },
                    );
                }
                KlineChartKind::Candles => {
                    let candle_width = chart.cell_width * 0.8;
                    let interval_ms = match &self.basis {
                        ChartBasis::Time(tf) => tf.to_milliseconds(),
                        ChartBasis::Tick(_) => 1000,
                    };

                    render_candles(
                        &self.chart_data.candles,
                        &self.chart_data.trades,
                        &self.basis,
                        chart.tick_size,
                        interval_ms,
                        frame,
                        earliest,
                        latest,
                        interval_to_x,
                        |frame, x_position, candle, _| {
                            draw_candle(
                                frame,
                                price_to_y,
                                candle_width,
                                palette,
                                x_position,
                                candle,
                            );
                        },
                    );
                }
            }

            chart.draw_last_price_line(frame, palette, region);
        });

        let crosshair = chart.cache.crosshair.draw(renderer, bounds_size, |frame| {
            if let Some(cursor_position) = cursor.position_in(bounds) {
                let (_, rounded_aggregation) =
                    chart.draw_crosshair(frame, theme, bounds_size, cursor_position, interaction);

                draw_crosshair_tooltip(
                    &self.chart_data.candles,
                    &self.basis,
                    &self.ticker_info,
                    frame,
                    palette,
                    rounded_aggregation,
                );
            }
        });

        vec![klines, crosshair]
    }

    fn mouse_interaction(
        &self,
        interaction: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match interaction {
            Interaction::Panning { .. } => mouse::Interaction::Grabbing,
            Interaction::Zoomin { .. } => mouse::Interaction::ZoomIn,
            Interaction::None | Interaction::Ruler { .. } => {
                if cursor.is_over(bounds) {
                    mouse::Interaction::Crosshair
                } else {
                    mouse::Interaction::default()
                }
            }
        }
    }
}

// ============================================================================
// HELPER TYPES AND FUNCTIONS
// ============================================================================

/// Helper struct for footprint trades
#[derive(Default)]
struct TradeGroup {
    buy_qty: f32,
    sell_qty: f32,
}

impl TradeGroup {
    fn total_qty(&self) -> f32 {
        self.buy_qty + self.sell_qty
    }

    fn delta_qty(&self) -> f32 {
        self.buy_qty - self.sell_qty
    }
}

/// Convert domain price to exchange price
#[inline]
fn domain_to_exchange_price(price: DomainPrice) -> Price {
    Price::from_units(price.units())
}

fn draw_footprint_candle(
    frame: &mut canvas::Frame,
    price_to_y: impl Fn(Price) -> f32,
    x_position: f32,
    candle_width: f32,
    candle: &Candle,
    palette: &Extended,
) {
    let y_open = price_to_y(domain_to_exchange_price(candle.open));
    let y_high = price_to_y(domain_to_exchange_price(candle.high));
    let y_low = price_to_y(domain_to_exchange_price(candle.low));
    let y_close = price_to_y(domain_to_exchange_price(candle.close));

    let body_color = if candle.close >= candle.open {
        palette.success.weak.color
    } else {
        palette.danger.weak.color
    };
    frame.fill_rectangle(
        Point::new(x_position - (candle_width / 8.0), y_open.min(y_close)),
        Size::new(candle_width / 4.0, (y_open - y_close).abs()),
        body_color,
    );

    let wick_color = if candle.close >= candle.open {
        palette.success.weak.color
    } else {
        palette.danger.weak.color
    };
    let marker_line = Stroke::with_color(
        Stroke {
            width: 1.0,
            ..Default::default()
        },
        wick_color.scale_alpha(0.6),
    );
    frame.stroke(
        &Path::line(
            Point::new(x_position, y_high),
            Point::new(x_position, y_low),
        ),
        marker_line,
    );
}

fn draw_candle(
    frame: &mut canvas::Frame,
    price_to_y: impl Fn(Price) -> f32,
    candle_width: f32,
    palette: &Extended,
    x_position: f32,
    candle: &Candle,
) {
    let y_open = price_to_y(domain_to_exchange_price(candle.open));
    let y_high = price_to_y(domain_to_exchange_price(candle.high));
    let y_low = price_to_y(domain_to_exchange_price(candle.low));
    let y_close = price_to_y(domain_to_exchange_price(candle.close));

    let body_color = if candle.close >= candle.open {
        palette.success.base.color
    } else {
        palette.danger.base.color
    };
    frame.fill_rectangle(
        Point::new(x_position - (candle_width / 2.0), y_open.min(y_close)),
        Size::new(candle_width, (y_open - y_close).abs()),
        body_color,
    );

    let wick_color = if candle.close >= candle.open {
        palette.success.base.color
    } else {
        palette.danger.base.color
    };
    frame.fill_rectangle(
        Point::new(x_position - (candle_width / 8.0), y_high),
        Size::new(candle_width / 4.0, (y_high - y_low).abs()),
        wick_color,
    );
}

fn render_candles<F>(
    candles: &[Candle],
    trades: &[Trade],
    basis: &ChartBasis,
    _tick_size: PriceStep,
    interval_ms: u64,
    frame: &mut canvas::Frame,
    earliest: u64,
    latest: u64,
    interval_to_x: impl Fn(u64) -> f32,
    draw_fn: F,
) where
    F: Fn(&mut canvas::Frame, f32, &Candle, &[Trade]),
{
    match basis {
        ChartBasis::Tick(_) => {
            let earliest_idx = earliest as usize;
            let latest_idx = latest as usize;

            candles
                .iter()
                .rev()
                .enumerate()
                .filter(|(index, _)| *index <= latest_idx && *index >= earliest_idx)
                .for_each(|(index, candle)| {
                    let x_position = interval_to_x(index as u64);

                    // Get trades for this candle by time range using binary search
                    let candle_start = candle.time.0;
                    let candle_end = candle.time.0 + interval_ms;

                    // Find start index using binary search
                    let start_idx = trades
                        .binary_search_by_key(&candle_start, |t| t.time.0)
                        .unwrap_or_else(|i| i);

                    // Find end index using binary search on the remaining slice
                    let end_idx = trades[start_idx..]
                        .binary_search_by_key(&candle_end, |t| t.time.0)
                        .map(|i| start_idx + i)
                        .unwrap_or_else(|i| start_idx + i);

                    let candle_trades = &trades[start_idx..end_idx];

                    draw_fn(frame, x_position, candle, candle_trades);
                });
        }
        ChartBasis::Time(_) => {
            if latest < earliest {
                return;
            }

            candles
                .iter()
                .filter(|c| c.time.0 >= earliest && c.time.0 <= latest)
                .for_each(|candle| {
                    let x_position = interval_to_x(candle.time.0);

                    // Get trades for this candle by time range using binary search
                    let candle_start = candle.time.0;
                    let candle_end = candle.time.0 + interval_ms;

                    // Find start index using binary search
                    let start_idx = trades
                        .binary_search_by_key(&candle_start, |t| t.time.0)
                        .unwrap_or_else(|i| i);

                    // Find end index using binary search on the remaining slice
                    let end_idx = trades[start_idx..]
                        .binary_search_by_key(&candle_end, |t| t.time.0)
                        .map(|i| start_idx + i)
                        .unwrap_or_else(|i| start_idx + i);

                    let candle_trades = &trades[start_idx..end_idx];

                    draw_fn(frame, x_position, candle, candle_trades);
                });
        }
    }
}

fn draw_all_npocs(
    candles: &[Candle],
    trades: &[Trade],
    basis: &ChartBasis,
    frame: &mut canvas::Frame,
    price_to_y: &impl Fn(Price) -> f32,
    interval_to_x: &impl Fn(u64) -> f32,
    candle_width: f32,
    cell_width: f32,
    cell_height: f32,
    tick_size: PriceStep,
    palette: &Extended,
    lookback: usize,
    _visible_earliest: u64,
    visible_latest: u64,
    cluster_kind: ClusterKind,
    spacing: ContentGaps,
    imb_study_on: bool,
) {
    // Calculate POCs for all candles
    let mut pocs: Vec<(usize, Price, f32)> = Vec::new(); // (candle_index, price, volume)

    for (idx, candle) in candles.iter().enumerate() {
        // Get trades for this candle using binary search
        let candle_start = candle.time.0;
        let candle_end = if idx + 1 < candles.len() {
            candles[idx + 1].time.0
        } else {
            candle.time.0 + 60000 // default 1 minute
        };

        // Find start index using binary search
        let start_idx = trades
            .binary_search_by_key(&candle_start, |t| t.time.0)
            .unwrap_or_else(|i| i);

        // Find end index using binary search on the remaining slice
        let end_idx = trades[start_idx..]
            .binary_search_by_key(&candle_end, |t| t.time.0)
            .map(|i| start_idx + i)
            .unwrap_or_else(|i| start_idx + i);

        let candle_trades = &trades[start_idx..end_idx];

        // Build volume profile for this candle
        let mut volume_profile: BTreeMap<Price, f32> = BTreeMap::new();
        for trade in candle_trades {
            let price_rounded = domain_to_exchange_price(trade.price).round_to_step(tick_size);
            *volume_profile.entry(price_rounded).or_insert(0.0) += trade.quantity.0 as f32;
        }

        // Find POC (price with max volume)
        if let Some((poc_price, poc_volume)) = volume_profile
            .iter()
            .max_by(|(_, v1), (_, v2)| v1.partial_cmp(v2).unwrap())
        {
            pocs.push((idx, *poc_price, *poc_volume));
        }
    }

    // Track naked POCs (POCs that haven't been revisited)
    let mut npocs: Vec<(usize, Price)> = Vec::new();

    for (idx, poc_price, _) in &pocs {
        let mut is_naked = true;

        // Check if price was revisited in next `lookback` candles
        for future_idx in (idx + 1)..(idx + 1 + lookback).min(candles.len()) {
            let future_candle = &candles[future_idx];

            // Check if POC price is within future candle's range
            let future_low = domain_to_exchange_price(future_candle.low);
            let future_high = domain_to_exchange_price(future_candle.high);
            if *poc_price >= future_low && *poc_price <= future_high {
                is_naked = false;
                break;
            }
        }

        if is_naked {
            npocs.push((*idx, *poc_price));
        }
    }

    // Draw nPOC lines
    let (_filled_color, naked_color) = (
        palette.background.strong.color,
        if palette.is_dark {
            palette.warning.weak.color.scale_alpha(0.5)
        } else {
            palette.warning.strong.color
        },
    );

    let line_height = cell_height.min(2.0);
    let bar_width_factor: f32 = 0.9;
    let inset = (cell_width * (1.0 - bar_width_factor)) / 2.0;

    let candle_lane_factor: f32 = match cluster_kind {
        ClusterKind::VolumeProfile | ClusterKind::DeltaProfile => 0.25,
        ClusterKind::BidAsk => 1.0,
        ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => 1.0,
    };

    let start_x_for = |cell_center_x: f32| -> f32 {
        match cluster_kind {
            ClusterKind::BidAsk => cell_center_x + (candle_width / 2.0) + spacing.candle_to_cluster,
            ClusterKind::VolumeProfile | ClusterKind::DeltaProfile => {
                let content_left = (cell_center_x - (cell_width / 2.0)) + inset;
                let candle_lane_left = content_left
                    + if imb_study_on {
                        candle_width + spacing.marker_to_candle
                    } else {
                        0.0
                    };
                candle_lane_left + candle_width * candle_lane_factor + spacing.candle_to_cluster
            }
            ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => {
                cell_center_x + (candle_width / 2.0) + spacing.candle_to_cluster
            }
        }
    };

    let rightmost_x = interval_to_x(visible_latest);

    for (candle_idx, npoc_price) in npocs {
        // Get candle time/position
        let candle_time = match basis {
            ChartBasis::Time(_) => candles[candle_idx].time.0,
            ChartBasis::Tick(_) => {
                let reverse_idx = candles.len() - 1 - candle_idx;
                reverse_idx as u64
            }
        };

        let start_x = interval_to_x(candle_time);
        let cell_center_x = start_x;
        let line_start_x = start_x_for(cell_center_x);
        let line_end_x = rightmost_x;

        let y = price_to_y(npoc_price);

        // Draw horizontal line from candle to right edge
        frame.fill_rectangle(
            Point::new(line_start_x, y - (line_height / 2.0)),
            Size::new(line_end_x - line_start_x, line_height),
            naked_color,
        );
    }
}

fn effective_cluster_qty(
    scaling: ClusterScaling,
    visible_max: f32,
    footprint: &BTreeMap<Price, TradeGroup>,
    cluster_kind: ClusterKind,
) -> f32 {
    let individual_max = match cluster_kind {
        ClusterKind::BidAsk => footprint
            .values()
            .map(|group| group.buy_qty.max(group.sell_qty))
            .fold(0.0_f32, f32::max),
        ClusterKind::DeltaProfile => footprint
            .values()
            .map(|group| (group.buy_qty - group.sell_qty).abs())
            .fold(0.0_f32, f32::max),
        ClusterKind::VolumeProfile => footprint
            .values()
            .map(|group| group.buy_qty + group.sell_qty)
            .fold(0.0_f32, f32::max),
        ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => footprint
            .values()
            .map(|group| group.buy_qty + group.sell_qty)
            .fold(0.0_f32, f32::max),
    };

    let safe = |v: f32| if v <= f32::EPSILON { 1.0 } else { v };

    match scaling {
        ClusterScaling::VisibleRange => safe(visible_max),
        ClusterScaling::Datapoint => safe(individual_max),
        ClusterScaling::Hybrid { weight } => {
            let w = weight.clamp(0.0, 1.0);
            safe(visible_max * w + individual_max * (1.0 - w))
        }
        ClusterScaling::Linear | ClusterScaling::Sqrt | ClusterScaling::Log => {
            // These are transformation modes, not max determination modes
            // Use visible_max as default
            safe(visible_max)
        }
    }
}

fn draw_clusters(
    frame: &mut canvas::Frame,
    price_to_y: impl Fn(Price) -> f32,
    x_position: f32,
    cell_width: f32,
    cell_height: f32,
    candle_width: f32,
    max_cluster_qty: f32,
    palette: &Extended,
    text_size: f32,
    tick_size: f32,
    show_text: bool,
    imbalance: Option<(usize, Option<usize>, bool)>,
    candle: &Candle,
    footprint: &BTreeMap<Price, TradeGroup>,
    cluster_kind: ClusterKind,
    spacing: ContentGaps,
) {
    let text_color = palette.background.weakest.text;

    let bar_width_factor: f32 = 0.9;
    let inset = (cell_width * (1.0 - bar_width_factor)) / 2.0;

    let cell_left = x_position - (cell_width / 2.0);
    let content_left = cell_left + inset;
    let content_right = x_position + (cell_width / 2.0) - inset;

    match cluster_kind {
        ClusterKind::VolumeProfile | ClusterKind::DeltaProfile => {
            let area = ProfileArea::new(
                content_left,
                content_right,
                candle_width,
                spacing,
                imbalance.is_some(),
            );
            let bar_alpha = if show_text { 0.25 } else { 1.0 };

            for (price, group) in footprint {
                let y = price_to_y(*price);

                match cluster_kind {
                    ClusterKind::VolumeProfile => {
                        super::draw_volume_bar(
                            frame,
                            area.bars_left,
                            y,
                            group.buy_qty,
                            group.sell_qty,
                            max_cluster_qty,
                            area.bars_width,
                            cell_height,
                            palette.success.base.color,
                            palette.danger.base.color,
                            bar_alpha,
                            true,
                        );

                        if show_text {
                            draw_cluster_text(
                                frame,
                                &abbr_large_numbers(group.total_qty()),
                                Point::new(area.bars_left, y),
                                text_size,
                                text_color,
                                Alignment::Start,
                                Alignment::Center,
                            );
                        }
                    }
                    ClusterKind::DeltaProfile => {
                        let delta = group.delta_qty();
                        if show_text {
                            draw_cluster_text(
                                frame,
                                &abbr_large_numbers(delta),
                                Point::new(area.bars_left, y),
                                text_size,
                                text_color,
                                Alignment::Start,
                                Alignment::Center,
                            );
                        }

                        let bar_width = (delta.abs() / max_cluster_qty) * area.bars_width;
                        if bar_width > 0.0 {
                            let color = if delta >= 0.0 {
                                palette.success.base.color.scale_alpha(bar_alpha)
                            } else {
                                palette.danger.base.color.scale_alpha(bar_alpha)
                            };
                            frame.fill_rectangle(
                                Point::new(area.bars_left, y - (cell_height / 2.0)),
                                Size::new(bar_width, cell_height),
                                color,
                            );
                        }
                    }
                    _ => {}
                }

                if let Some((threshold, color_scale, ignore_zeros)) = imbalance {
                    let step = PriceStep::from_f32(tick_size);
                    let higher_price =
                        Price::from_f32(price.to_f32() + tick_size).round_to_step(step);

                    let rect_w = ((area.imb_marker_width - 1.0) / 2.0).max(1.0);
                    let buyside_x = area.imb_marker_left + area.imb_marker_width - rect_w;
                    let sellside_x =
                        area.imb_marker_left + area.imb_marker_width - (2.0 * rect_w) - 1.0;

                    draw_imbalance_markers(
                        frame,
                        &price_to_y,
                        footprint,
                        *price,
                        group.sell_qty,
                        higher_price,
                        threshold as u8,
                        color_scale.is_some(),
                        ignore_zeros,
                        cell_height,
                        palette,
                        buyside_x,
                        sellside_x,
                        rect_w,
                    );
                }
            }

            draw_footprint_candle(
                frame,
                &price_to_y,
                area.candle_center_x,
                candle_width,
                candle,
                palette,
            );
        }
        ClusterKind::BidAsk => {
            let area = BidAskArea::new(
                x_position,
                content_left,
                content_right,
                candle_width,
                spacing,
            );

            let bar_alpha = if show_text { 0.25 } else { 1.0 };

            let imb_marker_reserve = if imbalance.is_some() {
                ((area.imb_marker_width - 1.0) / 2.0).max(1.0)
            } else {
                0.0
            };

            let right_max_x =
                area.bid_area_right - imb_marker_reserve - (2.0 * spacing.marker_to_bars);
            let right_area_width = (right_max_x - area.bid_area_left).max(0.0);

            let left_min_x =
                area.ask_area_left + imb_marker_reserve + (2.0 * spacing.marker_to_bars);
            let left_area_width = (area.ask_area_right - left_min_x).max(0.0);

            for (price, group) in footprint {
                let y = price_to_y(*price);

                if group.buy_qty > 0.0 && right_area_width > 0.0 {
                    if show_text {
                        draw_cluster_text(
                            frame,
                            &abbr_large_numbers(group.buy_qty),
                            Point::new(area.bid_area_left, y),
                            text_size,
                            text_color,
                            Alignment::Start,
                            Alignment::Center,
                        );
                    }

                    let bar_width = (group.buy_qty / max_cluster_qty) * right_area_width;
                    if bar_width > 0.0 {
                        frame.fill_rectangle(
                            Point::new(area.bid_area_left, y - (cell_height / 2.0)),
                            Size::new(bar_width, cell_height),
                            palette.success.base.color.scale_alpha(bar_alpha),
                        );
                    }
                }
                if group.sell_qty > 0.0 && left_area_width > 0.0 {
                    if show_text {
                        draw_cluster_text(
                            frame,
                            &abbr_large_numbers(group.sell_qty),
                            Point::new(area.ask_area_right, y),
                            text_size,
                            text_color,
                            Alignment::End,
                            Alignment::Center,
                        );
                    }

                    let bar_width = (group.sell_qty / max_cluster_qty) * left_area_width;
                    if bar_width > 0.0 {
                        frame.fill_rectangle(
                            Point::new(area.ask_area_right, y - (cell_height / 2.0)),
                            Size::new(-bar_width, cell_height),
                            palette.danger.base.color.scale_alpha(bar_alpha),
                        );
                    }
                }

                if let Some((threshold, color_scale, ignore_zeros)) = imbalance
                    && area.imb_marker_width > 0.0
                {
                    let step = PriceStep::from_f32(tick_size);
                    let higher_price =
                        Price::from_f32(price.to_f32() + tick_size).round_to_step(step);

                    let rect_width = ((area.imb_marker_width - 1.0) / 2.0).max(1.0);

                    let buyside_x = area.bid_area_right - rect_width - spacing.marker_to_bars;
                    let sellside_x = area.ask_area_left + spacing.marker_to_bars;

                    draw_imbalance_markers(
                        frame,
                        &price_to_y,
                        footprint,
                        *price,
                        group.sell_qty,
                        higher_price,
                        threshold as u8,
                        color_scale.is_some(),
                        ignore_zeros,
                        cell_height,
                        palette,
                        buyside_x,
                        sellside_x,
                        rect_width,
                    );
                }
            }

            draw_footprint_candle(
                frame,
                &price_to_y,
                area.candle_center_x,
                candle_width,
                candle,
                palette,
            );
        }
        ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => {
            // For simple cluster kinds, use BidAsk-style rendering
            let area = BidAskArea::new(
                x_position,
                content_left,
                content_right,
                candle_width,
                spacing,
            );

            let bar_alpha = if show_text { 0.25 } else { 1.0 };

            let imb_marker_reserve = if imbalance.is_some() {
                ((area.imb_marker_width - 1.0) / 2.0).max(1.0)
            } else {
                0.0
            };

            let right_max_x =
                area.bid_area_right - imb_marker_reserve - (2.0 * spacing.marker_to_bars);
            let right_area_width = (right_max_x - area.bid_area_left).max(0.0);

            let left_min_x =
                area.ask_area_left + imb_marker_reserve + (2.0 * spacing.marker_to_bars);
            let left_area_width = (area.ask_area_right - left_min_x).max(0.0);

            for (price, group) in footprint {
                let y = price_to_y(*price);

                if group.buy_qty > 0.0 && right_area_width > 0.0 {
                    if show_text {
                        draw_cluster_text(
                            frame,
                            &abbr_large_numbers(group.buy_qty),
                            Point::new(area.bid_area_left, y),
                            text_size,
                            text_color,
                            Alignment::Start,
                            Alignment::Center,
                        );
                    }

                    let bar_width = (group.buy_qty / max_cluster_qty) * right_area_width;
                    if bar_width > 0.0 {
                        frame.fill_rectangle(
                            Point::new(area.bid_area_left, y - (cell_height / 2.0)),
                            Size::new(bar_width, cell_height),
                            palette.success.base.color.scale_alpha(bar_alpha),
                        );
                    }
                }
                if group.sell_qty > 0.0 && left_area_width > 0.0 {
                    if show_text {
                        draw_cluster_text(
                            frame,
                            &abbr_large_numbers(group.sell_qty),
                            Point::new(area.ask_area_right, y),
                            text_size,
                            text_color,
                            Alignment::End,
                            Alignment::Center,
                        );
                    }

                    let bar_width = (group.sell_qty / max_cluster_qty) * left_area_width;
                    if bar_width > 0.0 {
                        frame.fill_rectangle(
                            Point::new(area.ask_area_right, y - (cell_height / 2.0)),
                            Size::new(-bar_width, cell_height),
                            palette.danger.base.color.scale_alpha(bar_alpha),
                        );
                    }
                }

                if let Some((threshold, color_scale, ignore_zeros)) = imbalance
                    && area.imb_marker_width > 0.0
                {
                    let step = PriceStep::from_f32(tick_size);
                    let higher_price =
                        Price::from_f32(price.to_f32() + tick_size).round_to_step(step);

                    let rect_width = ((area.imb_marker_width - 1.0) / 2.0).max(1.0);

                    let buyside_x = area.bid_area_right - rect_width - spacing.marker_to_bars;
                    let sellside_x = area.ask_area_left + spacing.marker_to_bars;

                    draw_imbalance_markers(
                        frame,
                        &price_to_y,
                        footprint,
                        *price,
                        group.sell_qty,
                        higher_price,
                        threshold as u8,
                        color_scale.is_some(),
                        ignore_zeros,
                        cell_height,
                        palette,
                        buyside_x,
                        sellside_x,
                        rect_width,
                    );
                }
            }

            draw_footprint_candle(
                frame,
                &price_to_y,
                area.candle_center_x,
                candle_width,
                candle,
                palette,
            );
        }
    }
}

fn draw_imbalance_markers(
    frame: &mut canvas::Frame,
    price_to_y: &impl Fn(Price) -> f32,
    footprint: &BTreeMap<Price, TradeGroup>,
    price: Price,
    sell_qty: f32,
    higher_price: Price,
    threshold: u8,
    color_scale: bool,
    ignore_zeros: bool,
    cell_height: f32,
    palette: &Extended,
    buyside_x: f32,
    sellside_x: f32,
    rect_width: f32,
) {
    if ignore_zeros && sell_qty <= 0.0 {
        return;
    }

    if let Some(group) = footprint.get(&higher_price) {
        let diagonal_buy_qty = group.buy_qty;

        if ignore_zeros && diagonal_buy_qty <= 0.0 {
            return;
        }

        let rect_height = cell_height / 2.0;

        let alpha_from_ratio = |ratio: f32| -> f32 {
            if color_scale {
                // Smooth color scale based on ratio
                (0.2 + 0.8 * (ratio - 1.0).min(1.0)).min(1.0)
            } else {
                1.0
            }
        };

        if diagonal_buy_qty >= sell_qty {
            let required_qty = sell_qty * (100 + threshold) as f32 / 100.0;
            if diagonal_buy_qty > required_qty {
                let ratio = diagonal_buy_qty / required_qty;
                let alpha = alpha_from_ratio(ratio);

                let y = price_to_y(higher_price);
                frame.fill_rectangle(
                    Point::new(buyside_x, y - (rect_height / 2.0)),
                    Size::new(rect_width, rect_height),
                    palette.success.weak.color.scale_alpha(alpha),
                );
            }
        } else {
            let required_qty = diagonal_buy_qty * (100 + threshold) as f32 / 100.0;
            if sell_qty > required_qty {
                let ratio = sell_qty / required_qty;
                let alpha = alpha_from_ratio(ratio);

                let y = price_to_y(price);
                frame.fill_rectangle(
                    Point::new(sellside_x, y - (rect_height / 2.0)),
                    Size::new(rect_width, rect_height),
                    palette.danger.weak.color.scale_alpha(alpha),
                );
            }
        }
    }
}

impl ContentGaps {
    fn from_view(candle_width: f32, scaling: f32) -> Self {
        let px = |p: f32| p / scaling;
        let base = (candle_width * 0.2).max(px(2.0));
        Self {
            marker_to_candle: base,
            candle_to_cluster: base,
            marker_to_bars: px(2.0),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ContentGaps {
    /// Space between imb. markers candle body
    marker_to_candle: f32,
    /// Space between candle body and clusters
    candle_to_cluster: f32,
    /// Inner space reserved between imb. markers and clusters (used for BidAsk)
    marker_to_bars: f32,
}

fn draw_cluster_text(
    frame: &mut canvas::Frame,
    text: &str,
    position: Point,
    text_size: f32,
    color: iced::Color,
    align_x: Alignment,
    align_y: Alignment,
) {
    frame.fill_text(canvas::Text {
        content: text.to_string(),
        position,
        size: iced::Pixels(text_size),
        color,
        align_x: align_x.into(),
        align_y: align_y.into(),
        font: style::AZERET_MONO,
        ..canvas::Text::default()
    });
}

fn draw_crosshair_tooltip(
    candles: &[Candle],
    basis: &ChartBasis,
    ticker_info: &FuturesTickerInfo,
    frame: &mut canvas::Frame,
    palette: &Extended,
    at_interval: u64,
) {
    let candle_opt = match basis {
        ChartBasis::Time(_) => candles
            .iter()
            .find(|c| c.time.0 == at_interval)
            .or_else(|| {
                if candles.is_empty() {
                    None
                } else {
                    let last = candles.last()?;
                    if at_interval > last.time.0 {
                        Some(last)
                    } else {
                        None
                    }
                }
            }),
        ChartBasis::Tick(tick_count) => {
            let index = (at_interval / u64::from(*tick_count)) as usize;
            if index < candles.len() {
                Some(&candles[candles.len() - 1 - index])
            } else {
                None
            }
        }
    };

    if let Some(candle) = candle_opt {
        let change_pct = ((candle.close.to_f32() - candle.open.to_f32()) / candle.open.to_f32()) * 100.0;
        let change_color = if change_pct >= 0.0 {
            palette.success.base.color
        } else {
            palette.danger.base.color
        };

        let base_color = palette.background.base.text;
        let precision = count_decimals(ticker_info.tick_size);

        let open_str = format!("{:.prec$}", candle.open.to_f32(), prec = precision);
        let high_str = format!("{:.prec$}", candle.high.to_f32(), prec = precision);
        let low_str = format!("{:.prec$}", candle.low.to_f32(), prec = precision);
        let close_str = format!("{:.prec$}", candle.close.to_f32(), prec = precision);
        let pct_str = format!("{change_pct:+.2}%");

        let segments = [
            ("O", base_color, false),
            (&open_str, change_color, true),
            ("H", base_color, false),
            (&high_str, change_color, true),
            ("L", base_color, false),
            (&low_str, change_color, true),
            ("C", base_color, false),
            (&close_str, change_color, true),
            (&pct_str, change_color, true),
        ];

        let total_width: f32 = segments
            .iter()
            .map(|(s, _, _)| s.len() as f32 * (TEXT_SIZE * 0.8))
            .sum();

        let position = Point::new(8.0, 8.0);

        let tooltip_rect = Rectangle {
            x: position.x,
            y: position.y,
            width: total_width,
            height: 16.0,
        };

        frame.fill_rectangle(
            tooltip_rect.position(),
            tooltip_rect.size(),
            palette.background.weakest.color.scale_alpha(0.9),
        );

        let mut x = position.x;
        for (text, seg_color, is_value) in segments {
            frame.fill_text(canvas::Text {
                content: text.to_string(),
                position: Point::new(x, position.y),
                size: iced::Pixels(12.0),
                color: seg_color,
                font: style::AZERET_MONO,
                ..canvas::Text::default()
            });
            x += text.len() as f32 * 8.0;
            x += if is_value { 6.0 } else { 2.0 };
        }
    }
}

struct ProfileArea {
    imb_marker_left: f32,
    imb_marker_width: f32,
    bars_left: f32,
    bars_width: f32,
    candle_center_x: f32,
}

impl ProfileArea {
    fn new(
        content_left: f32,
        content_right: f32,
        candle_width: f32,
        gaps: ContentGaps,
        has_imbalance: bool,
    ) -> Self {
        let candle_lane_left = if has_imbalance {
            content_left + candle_width + gaps.marker_to_candle
        } else {
            content_left
        };
        let candle_lane_width = candle_width * 0.25;

        let bars_left = candle_lane_left + candle_lane_width + gaps.candle_to_cluster;
        let bars_width = (content_right - bars_left).max(0.0);

        let candle_center_x = candle_lane_left + (candle_lane_width / 2.0);

        Self {
            imb_marker_left: content_left,
            imb_marker_width: if has_imbalance { candle_width } else { 0.0 },
            bars_left,
            bars_width,
            candle_center_x,
        }
    }
}

struct BidAskArea {
    bid_area_left: f32,
    bid_area_right: f32,
    ask_area_left: f32,
    ask_area_right: f32,
    candle_center_x: f32,
    imb_marker_width: f32,
}

impl BidAskArea {
    fn new(
        x_position: f32,
        content_left: f32,
        content_right: f32,
        candle_width: f32,
        spacing: ContentGaps,
    ) -> Self {
        let candle_body_width = candle_width * 0.25;

        let candle_left = x_position - (candle_body_width / 2.0);
        let candle_right = x_position + (candle_body_width / 2.0);

        let ask_area_right = candle_left - spacing.candle_to_cluster;
        let bid_area_left = candle_right + spacing.candle_to_cluster;

        Self {
            bid_area_left,
            bid_area_right: content_right,
            ask_area_left: content_left,
            ask_area_right,
            candle_center_x: x_position,
            imb_marker_width: candle_width,
        }
    }
}

#[inline]
fn should_show_text(cell_height_unscaled: f32, cell_width_unscaled: f32, min_w: f32) -> bool {
    cell_height_unscaled > 8.0 && cell_width_unscaled > min_w
}
