//! Heatmap Chart - Order Book Depth Visualization
//!
//! This module provides a volume heatmap chart that displays:
//! - Order book depth as a colored heatmap over time
//! - Trade executions overlaid as circles
//! - Volume indicators showing buy/sell volume
//! - Volume profile studies (POC, VAH, VAL)
//!
//! ## Architecture
//!
//! The heatmap follows the new architecture:
//! - **ChartData**: Contains historical depth snapshots and trades
//! - **HeatmapData**: Aggregated depth runs and trade buckets for efficient rendering
//! - **Replay Mode**: Supports adding new snapshots dynamically
//!
//! ## Data Flow
//!
//! ```text
//! ChartData (depth_snapshots, trades)
//!     ↓
//! HeatmapData (depth_by_price, trades_by_time)
//!     ↓
//! Rendering (colored cells, circles, volume bars)
//! ```

pub mod data;
mod render;
mod studies;
pub mod trades;

use crate::chart::{
    Chart, PlotLimits, ViewState,
    core::tokens as chart_tokens,
    drawing::{ChartDrawingAccess, DrawingManager},
    scale::linear::PriceInfoLabel,
};
use crate::modals::pane::settings::study_config as study;

use data::{HeatmapData, QtyScale, VolumeProfile, VolumeProfileKey};

use ::data::util::count_decimals;
use ::data::{
    ChartBasis, ChartData, DepthSnapshot, HeatmapIndicator, Price as DataPrice,
    Trade as DomainTrade, ViewConfig,
};
use exchange::FuturesTickerInfo;

use iced::Vector;

use enum_map::EnumMap;
use std::cell::{Cell, RefCell};
use std::time::Instant;

// Re-export types from submodules
pub use render::HeatmapStudy;
pub use trades::TradeRenderingMode;

// ── Constants - Visual Configuration ──────────────────────────────────
// Cell size limits are defined centrally in chart::core::tokens::heatmap.

use chart_tokens::heatmap::{
    DEFAULT_CELL_WIDTH, MAX_CELL_HEIGHT, MAX_CELL_WIDTH, MIN_CELL_HEIGHT, MIN_CELL_WIDTH,
};

// ── Chart Trait Implementation ────────────────────────────────────────

impl Chart for HeatmapChart {
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
        self.invalidate(None);
    }

    fn interval_keys(&self) -> Option<Vec<u64>> {
        None
    }

    fn autoscaled_coords(&self) -> Vector {
        let chart = self.state();
        Vector::new(
            0.5 * (chart.bounds.width / chart.scaling)
                - (90.0 / chart.scaling),
            chart.translation.y,
        )
    }

    fn supports_fit_autoscaling(&self) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        !self.chart_data.has_depth() && !self.chart_data.has_trades()
    }

    fn plot_limits(&self) -> PlotLimits {
        PlotLimits {
            max_cell_width: MAX_CELL_WIDTH,
            min_cell_width: MIN_CELL_WIDTH,
            max_cell_height: MAX_CELL_HEIGHT,
            min_cell_height: MIN_CELL_HEIGHT,
            default_cell_width: DEFAULT_CELL_WIDTH,
        }
    }

    fn drawings(&self) -> Option<&DrawingManager> {
        Some(&self.drawings)
    }
}

// ── Domain Types ──────────────────────────────────────────────────────

/// Visual configuration for heatmap display
#[derive(Debug, Clone, Copy)]
pub struct VisualConfig {
    /// Minimum order size to display in contracts (filter small orders)
    pub order_size_filter: f32,
    /// Minimum trade size to display in contracts (NOT dollar amount)
    /// Example: 5.0 = only show trades >= 5 contracts
    pub trade_size_filter: f32,
    /// Trade circle size scaling (None = fixed size)
    pub trade_size_scale: Option<u16>,
    /// Trade rendering mode (Sparse/Dense/Auto)
    pub trade_rendering_mode: TradeRenderingMode,
    /// Maximum number of trade circles to render (performance limit)
    pub max_trade_markers: usize,
}

impl Default for VisualConfig {
    fn default() -> Self {
        Self {
            order_size_filter: 0.0,
            trade_size_filter: 0.0,
            trade_size_scale: Some(100),
            trade_rendering_mode: TradeRenderingMode::Auto,
            max_trade_markers: 10_000, // Performance limit: max 10k circles
        }
    }
}

// ── Main Chart Structure ──────────────────────────────────────────────

/// Heatmap Chart - Volume Heatmap with Order Flow
///
/// This chart visualizes orderbook depth over time as a colored heatmap,
/// with trade executions overlaid as circles.
pub struct HeatmapChart {
    /// View state (camera, bounds, caching)
    chart: ViewState,

    /// Source data (trades + depth snapshots)
    chart_data: ChartData,

    /// Chart basis (time or tick aggregation)
    pub(crate) basis: ChartBasis,

    /// Ticker information (tick size, decimals, etc.)
    ticker_info: FuturesTickerInfo,

    /// Processed heatmap data (depth runs + trade buckets)
    pub(crate) heatmap_data: HeatmapData,

    /// Active indicators (presence tracked as bool)
    pub(crate) indicators: EnumMap<HeatmapIndicator, bool>,

    /// Visual configuration (filters, scaling)
    pub(crate) visual_config: VisualConfig,

    /// Study configurator UI state
    study_configurator: study::Configurator<HeatmapStudy>,

    /// Active studies
    pub studies: Vec<HeatmapStudy>,

    /// Last update timestamp (for cache invalidation)
    last_tick: Instant,

    /// Drawing manager for chart annotations
    drawings: DrawingManager,

    /// Cached qty_scales to avoid recomputation when viewport hasn't changed
    qty_scale_cache: Cell<Option<(u64, u64, i64, i64, QtyScale)>>,

    /// Cached volume profile to avoid recomputation in draw closure
    volume_profile_cache: RefCell<Option<(VolumeProfileKey, VolumeProfile)>>,
}

impl HeatmapChart {
    /// Create a new heatmap chart from chart data
    pub fn from_chart_data(
        chart_data: ChartData,
        basis: ChartBasis,
        ticker_info: FuturesTickerInfo,
        layout: ViewConfig,
        indicators: &[HeatmapIndicator],
        studies: Vec<HeatmapStudy>,
    ) -> Self {
        let tick_size = DataPrice::from_f32(ticker_info.tick_size);
        let tick_step = exchange::util::PriceStep::from_f32(ticker_info.tick_size);

        // Initialize indicator map
        let mut indicators_map = EnumMap::default();
        for &indicator in indicators {
            indicators_map[indicator] = true;
        }

        // Build heatmap data from chart_data
        let mut heatmap_data = HeatmapData::new_with_candles(&chart_data.candles, basis);

        // Process depth snapshots if available
        if let Some(depth_snapshots) = &chart_data.depth_snapshots {
            log::info!(
                "Processing {} depth snapshots for heatmap...",
                depth_snapshots.len()
            );
            let total = depth_snapshots.len();
            let start_time = std::time::Instant::now();

            for (idx, snapshot) in depth_snapshots.iter().enumerate() {
                heatmap_data.add_depth_snapshot(snapshot, basis, tick_size);

                if (idx + 1) % 1000 == 0 || idx + 1 == total {
                    let elapsed = start_time.elapsed().as_secs_f32();
                    let rate = (idx + 1) as f32 / elapsed;
                    log::info!(
                        "  Processed {}/{} depth snapshots ({:.1}% - {:.0} snapshots/sec)",
                        idx + 1,
                        total,
                        ((idx + 1) as f32 / total as f32) * 100.0,
                        rate
                    );
                }
            }

            let total_time = start_time.elapsed();
            log::info!(
                "Depth processing complete in {:.2}s",
                total_time.as_secs_f32()
            );
        }

        // Process trades
        log::info!(
            "Processing {} trades for heatmap...",
            chart_data.trades.len()
        );
        for trade in &chart_data.trades {
            heatmap_data.add_trade(trade, basis, tick_size);
        }
        log::info!("Trade processing complete");

        // Calculate initial price from best bid/ask or trades
        let base_price = if let Some(depth_snapshots) = &chart_data.depth_snapshots {
            depth_snapshots
                .last()
                .and_then(|s| s.mid_price())
                .unwrap_or_else(|| DataPrice::from_f32(0.0))
        } else if let Some(trade) = chart_data.trades.last() {
            trade.price
        } else {
            DataPrice::from_f32(0.0)
        };

        // Calculate latest X coordinate
        let latest_x = if let Some(depth_snapshots) = &chart_data.depth_snapshots {
            depth_snapshots
                .last()
                .map(|s| s.time.to_millis())
                .unwrap_or(0)
        } else {
            chart_data
                .trades
                .last()
                .map(|t| t.time.to_millis())
                .unwrap_or(0)
        };

        // Initialize view state
        let view_state = ViewState::new(
            basis,
            tick_step,
            count_decimals(ticker_info.tick_size),
            ticker_info,
            ViewConfig {
                splits: layout.splits,
                autoscale: Some(::data::Autoscale::CenterLatest),
                side_splits: layout.side_splits,
            },
            DEFAULT_CELL_WIDTH,
            4.0,
        );

        let mut chart = HeatmapChart {
            chart: view_state,
            chart_data,
            basis,
            ticker_info,
            heatmap_data,
            indicators: indicators_map,
            visual_config: VisualConfig::default(),
            study_configurator: study::Configurator::new(),
            studies,
            last_tick: Instant::now(),
            drawings: DrawingManager::new(),
            qty_scale_cache: Cell::new(None),
            volume_profile_cache: RefCell::new(None),
        };

        // Set initial price and position
        chart.chart.base_price_y = exchange::util::Price::from_units(base_price.units());
        chart.chart.latest_x = latest_x;
        chart.chart.last_price = Some(PriceInfoLabel::Neutral(exchange::util::Price::from_units(
            base_price.units(),
        )));

        chart
    }

    /// Update heatmap from replay mode
    pub fn update_from_replay(&mut self, depth: &DepthSnapshot, trades: &[DomainTrade]) {
        let tick_size = DataPrice::from_f32(self.ticker_info.tick_size);

        self.heatmap_data
            .add_depth_snapshot(depth, self.basis, tick_size);

        for trade in trades {
            self.heatmap_data.add_trade(trade, self.basis, tick_size);
        }

        if let Some(mid_price) = depth.mid_price() {
            self.chart.base_price_y = exchange::util::Price::from_units(mid_price.units());
            self.chart.last_price = Some(PriceInfoLabel::Neutral(
                exchange::util::Price::from_units(mid_price.units()),
            ));
        }

        self.chart.latest_x = depth.time.to_millis();
        self.invalidate(Some(Instant::now()));
    }

    /// Rebuild the chart from scratch with the given trades.
    ///
    /// Clears all existing trades and heatmap trade data, then
    /// replays the trades. Used during replay seek to ensure
    /// the chart exactly represents `[start, position]`.
    pub fn rebuild_from_trades(&mut self, trades: &[DomainTrade]) {
        self.chart_data.trades.clear();
        self.heatmap_data.clear_trades();

        let tick_size = DataPrice::from_f32(self.ticker_info.tick_size);
        for trade in trades {
            self.chart_data.trades.push(*trade);
            self.heatmap_data.add_trade(trade, self.basis, tick_size);
        }

        // Update latest X and base price from last trade
        if let Some(last) = trades.last() {
            self.chart.latest_x = last.time.to_millis();
            self.chart.base_price_y = exchange::util::Price::from_units(last.price.units());
            self.chart.last_price = Some(PriceInfoLabel::Neutral(
                exchange::util::Price::from_units(last.price.units()),
            ));
        }

        self.invalidate(Some(Instant::now()));
    }

    /// Append a single trade during replay.
    ///
    /// Pushes the trade to internal `chart_data`, updates the
    /// heatmap data structures, and invalidates rendering caches.
    pub fn append_trade(&mut self, trade: &DomainTrade) {
        let tick_size = DataPrice::from_f32(self.ticker_info.tick_size);

        self.chart_data.trades.push(*trade);
        self.heatmap_data.add_trade(trade, self.basis, tick_size);

        // Update latest X and base price
        self.chart.latest_x = trade.time.to_millis();
        self.chart.base_price_y = exchange::util::Price::from_units(trade.price.units());
        self.chart.last_price = Some(PriceInfoLabel::Neutral(exchange::util::Price::from_units(
            trade.price.units(),
        )));
    }

    /// Get current visual configuration
    pub fn visual_config(&self) -> VisualConfig {
        self.visual_config
    }

    /// Set visual configuration
    pub fn set_visual_config(&mut self, visual_config: VisualConfig) {
        self.visual_config = visual_config;
        self.invalidate(Some(Instant::now()));
    }

    /// Change chart basis (time or tick aggregation)
    pub fn set_basis(&mut self, basis: ChartBasis) {
        self.basis = basis;
        self.chart.basis = basis;

        let tick_size = DataPrice::from_f32(self.ticker_info.tick_size);
        let mut heatmap_data = HeatmapData::new_with_candles(&self.chart_data.candles, basis);

        if let Some(depth_snapshots) = &self.chart_data.depth_snapshots {
            for snapshot in depth_snapshots {
                heatmap_data.add_depth_snapshot(snapshot, basis, tick_size);
            }
        }

        for trade in &self.chart_data.trades {
            heatmap_data.add_trade(trade, basis, tick_size);
        }

        self.heatmap_data = heatmap_data;

        let chart = &mut self.chart;
        chart.translation = Vector::new(
            0.5 * (chart.bounds.width / chart.scaling) - (90.0 / chart.scaling),
            0.0,
        );

        self.invalidate(None);
    }

    /// Get basis interval in milliseconds (None for tick basis)
    pub fn basis_interval(&self) -> Option<u64> {
        match self.basis {
            ChartBasis::Time(timeframe) => Some(timeframe.to_milliseconds()),
            ChartBasis::Tick(_) => None,
        }
    }

    /// Get chart layout configuration
    pub fn chart_layout(&self) -> ViewConfig {
        self.chart.layout()
    }

    /// Change tick size (price aggregation)
    pub fn change_tick_size(&mut self, new_tick_size: f32) {
        let chart_state = self.mut_state();

        let step = exchange::util::PriceStep::from_f32(new_tick_size);

        chart_state.cell_height = 4.0;
        chart_state.tick_size = step;
        chart_state.decimals = count_decimals(new_tick_size);

        let tick_size = DataPrice::from_f32(new_tick_size);
        let mut heatmap_data = HeatmapData::new_with_candles(&self.chart_data.candles, self.basis);

        if let Some(depth_snapshots) = &self.chart_data.depth_snapshots {
            for snapshot in depth_snapshots {
                heatmap_data.add_depth_snapshot(snapshot, self.basis, tick_size);
            }
        }

        for trade in &self.chart_data.trades {
            heatmap_data.add_trade(trade, self.basis, tick_size);
        }

        self.heatmap_data = heatmap_data;
    }

    /// Get current tick size
    pub fn tick_size(&self) -> f32 {
        self.chart.tick_size.to_f32_lossy()
    }

    /// Invalidate caches and trigger redraw
    pub fn invalidate(&mut self, now: Option<Instant>) {
        let chart = &mut self.chart;

        if chart.layout.autoscale.is_some() {
            chart.translation = Vector::new(
                0.5 * (chart.bounds.width / chart.scaling) - (90.0 / chart.scaling),
                0.0,
            );
        }

        chart.cache.clear_all();
        self.qty_scale_cache.set(None);
        self.volume_profile_cache.borrow_mut().take();

        if let Some(t) = now {
            self.last_tick = t;
        }
    }

    /// Get last update time
    pub fn last_update(&self) -> Instant {
        self.last_tick
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
}

impl ChartDrawingAccess for HeatmapChart {
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

impl HeatmapChart {
    /// Calculate quantity scales for rendering, with viewport-based caching
    pub(crate) fn calc_qty_scales(
        &self,
        earliest: u64,
        latest: u64,
        highest: DataPrice,
        lowest: DataPrice,
    ) -> QtyScale {
        let key = (earliest, latest, highest.units(), lowest.units());

        if let Some((ce, cl, ch, clo, cached)) = self.qty_scale_cache.get() {
            if ce == key.0 && cl == key.1 && ch == key.2 && clo == key.3 {
                return cached;
            }
        }

        let max_trade_qty = self.heatmap_data.max_trade_qty_in_range(earliest, latest);
        let max_aggr_volume = self.heatmap_data.max_aggr_volume_in_range(earliest, latest);
        let max_depth_qty = self.heatmap_data.max_depth_qty_in_range(
            earliest,
            latest,
            highest,
            lowest,
            self.visual_config.order_size_filter,
        );

        let result = QtyScale {
            max_trade_qty,
            max_aggr_volume,
            max_depth_qty,
        };

        self.qty_scale_cache
            .set(Some((key.0, key.1, key.2, key.3, result)));

        result
    }

    /// Get or compute the volume profile, using a RefCell cache keyed
    /// on the time range and tick parameters. Callable from `&self`.
    pub(crate) fn get_or_compute_volume_profile(
        &self,
        time_range: std::ops::RangeInclusive<u64>,
        first_tick: DataPrice,
        last_tick: DataPrice,
        step_units: i64,
        num_ticks: usize,
    ) {
        let key = VolumeProfileKey {
            earliest: *time_range.start(),
            latest: *time_range.end(),
            first_tick_units: first_tick.units(),
            last_tick_units: last_tick.units(),
            step_units,
        };

        let needs_recompute = self
            .volume_profile_cache
            .borrow()
            .as_ref()
            .map_or(true, |(cached_key, _)| *cached_key != key);

        if needs_recompute {
            let profile = self.heatmap_data.compute_volume_profile(
                time_range,
                first_tick,
                last_tick,
                step_units,
                num_ticks,
            );
            *self.volume_profile_cache.borrow_mut() = Some((key, profile));
        }
    }
}
