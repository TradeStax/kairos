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

use super::{
    Chart, Interaction, Message, PlotConstants, ViewState, scale::linear::PriceInfoLabel,
};
use crate::{
    modal::pane::settings::study,
    style,
};

use data::{
    ChartBasis, ChartData, DepthSnapshot, HeatmapIndicator, Price as DataPrice,
    Trade as DomainTrade, ViewConfig,
};
use data::util::{abbr_large_numbers, count_decimals};
use exchange::FuturesTickerInfo;

use iced::widget::canvas::{self, Event, Geometry, Path};
use iced::{
    Color, Element, Point, Rectangle, Renderer, Size, Theme, Vector, mouse,
    theme::palette::Extended,
};

use enum_map::EnumMap;
use std::collections::BTreeMap;
use std::time::Instant;

// =============================================================================
// Constants - Visual Configuration
// =============================================================================

/// Minimum chart scaling factor
const MIN_SCALING: f32 = 0.6;
/// Maximum chart scaling factor
const MAX_SCALING: f32 = 1.2;

/// Maximum cell width in pixels
const MAX_CELL_WIDTH: f32 = 12.0;
/// Minimum cell width in pixels
const MIN_CELL_WIDTH: f32 = 1.0;

/// Maximum cell height in pixels
const MAX_CELL_HEIGHT: f32 = 10.0;
/// Minimum cell height in pixels
const MIN_CELL_HEIGHT: f32 = 1.0;

/// Default cell width
const DEFAULT_CELL_WIDTH: f32 = 3.0;

/// Tooltip dimensions
const TOOLTIP_WIDTH: f32 = 198.0;
const TOOLTIP_HEIGHT: f32 = 66.0;
const TOOLTIP_PADDING: f32 = 12.0;

/// Maximum trade circle radius
const MAX_CIRCLE_RADIUS: f32 = 16.0;

// =============================================================================
// Chart Trait Implementation
// =============================================================================

impl Chart for HeatmapChart {
    type IndicatorKind = HeatmapIndicator;

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

    fn view_indicators(&'_ self, _indicators: &[Self::IndicatorKind]) -> Vec<Element<'_, Message>> {
        vec![]
    }

    fn visible_timerange(&self) -> Option<(u64, u64)> {
        let chart = self.state();
        let region = chart.visible_region(chart.bounds.size());

        if region.width == 0.0 {
            return None;
        }

        Some((
            chart.x_to_interval(region.x),
            chart.x_to_interval(region.x + region.width),
        ))
    }

    fn interval_keys(&self) -> Option<Vec<u64>> {
        None
    }

    fn autoscaled_coords(&self) -> Vector {
        let chart = self.state();
        Vector::new(
            0.5 * (chart.bounds.width / chart.scaling) - (90.0 / chart.scaling),
            chart.translation.y,
        )
    }

    fn supports_fit_autoscaling(&self) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        !self.chart_data.has_depth() && !self.chart_data.has_trades()
    }
}

impl PlotConstants for HeatmapChart {
    fn min_scaling(&self) -> f32 {
        MIN_SCALING
    }

    fn max_scaling(&self) -> f32 {
        MAX_SCALING
    }

    fn max_cell_width(&self) -> f32 {
        MAX_CELL_WIDTH
    }

    fn min_cell_width(&self) -> f32 {
        MIN_CELL_WIDTH
    }

    fn max_cell_height(&self) -> f32 {
        MAX_CELL_HEIGHT
    }

    fn min_cell_height(&self) -> f32 {
        MIN_CELL_HEIGHT
    }

    fn default_cell_width(&self) -> f32 {
        DEFAULT_CELL_WIDTH
    }
}

// =============================================================================
// Domain Types
// =============================================================================

/// Indicator data (currently just volume, but extensible)
#[derive(Default)]
enum IndicatorData {
    #[default]
    Volume,
}

// Re-export HeatmapStudy and ProfileKind from data module
pub use data::domain::chart_ui_types::heatmap::{HeatmapStudy, ProfileKind};

/// Visual configuration for heatmap display
#[derive(Debug, Clone, Copy)]
pub struct VisualConfig {
    /// Minimum order size to display (filter small orders)
    pub order_size_filter: f32,
    /// Minimum trade size to display (filter small trades)
    pub trade_size_filter: f32,
    /// Trade circle size scaling (None = fixed size)
    pub trade_size_scale: Option<u16>,
    /// Depth coalescing strategy (unused currently)
    pub coalescing: Option<CoalesceKind>,
}

/// Type alias for backward compatibility
pub type Config = VisualConfig;

impl Default for VisualConfig {
    fn default() -> Self {
        Self {
            order_size_filter: 0.0,
            trade_size_filter: 0.0,
            trade_size_scale: Some(100),
            coalescing: None,
        }
    }
}

/// Coalescing strategy for depth runs (future enhancement)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoalesceKind {
    Aggressive,
    Conservative,
}

// =============================================================================
// Internal Data Structures
// =============================================================================

/// A single depth run - continuous orderbook presence at a price level
#[derive(Debug, Clone)]
struct DepthRun {
    /// Start time of depth presence
    start_time: u64,
    /// End time of depth presence
    until_time: u64,
    /// Quantity at this price level
    qty: f32,
    /// True if bid, false if ask
    is_bid: bool,
}

impl DepthRun {
    fn qty(&self) -> f32 {
        self.qty
    }
}

/// Trade data point for a time bucket
#[derive(Debug, Clone)]
struct TradeDataPoint {
    /// Grouped trades by price
    grouped_trades: Vec<GroupedTrade>,
    /// Total buy volume in bucket
    buy_volume: f32,
    /// Total sell volume in bucket
    sell_volume: f32,
}

impl Default for TradeDataPoint {
    fn default() -> Self {
        Self {
            grouped_trades: Vec::new(),
            buy_volume: 0.0,
            sell_volume: 0.0,
        }
    }
}

/// A grouped trade at a specific price level
#[derive(Debug, Clone, Copy)]
struct GroupedTrade {
    price: DataPrice,
    qty: f32,
    is_sell: bool,
}

/// Aggregated heatmap data structure
///
/// This structure organizes historical depth snapshots and trades
/// into efficient lookup structures for rendering.
struct HeatmapData {
    /// Price level -> depth runs over time
    /// Key: price in units (i64), Value: list of depth runs
    depth_by_price: BTreeMap<i64, Vec<DepthRun>>,

    /// Time bucket -> aggregated trades
    /// Key: bucket time (u64), Value: trade data
    trades_by_time: BTreeMap<u64, TradeDataPoint>,

    /// Latest depth snapshot timestamp
    latest_depth_time: u64,
}

impl HeatmapData {
    /// Create empty heatmap data
    fn new() -> Self {
        Self {
            depth_by_price: BTreeMap::new(),
            trades_by_time: BTreeMap::new(),
            latest_depth_time: 0,
        }
    }

    /// Add a depth snapshot to the heatmap
    ///
    /// This processes the snapshot into depth runs, bucketing by the chart basis.
    /// Each price level gets a run for the duration of the bucket.
    fn add_depth_snapshot(&mut self, snapshot: &DepthSnapshot, basis: ChartBasis, _tick_size: DataPrice) {
        let time = snapshot.time.to_millis();
        let bucket_time = Self::bucket_time(time, basis);
        let bucket_duration = Self::bucket_duration(basis);

        // Process bids (buy orders)
        for (price, qty) in &snapshot.bids {
            let price_units = price.to_units();
            self.depth_by_price
                .entry(price_units)
                .or_insert_with(Vec::new)
                .push(DepthRun {
                    start_time: bucket_time,
                    until_time: bucket_time + bucket_duration,
                    qty: qty.value() as f32,
                    is_bid: true,
                });
        }

        // Process asks (sell orders)
        for (price, qty) in &snapshot.asks {
            let price_units = price.to_units();
            self.depth_by_price
                .entry(price_units)
                .or_insert_with(Vec::new)
                .push(DepthRun {
                    start_time: bucket_time,
                    until_time: bucket_time + bucket_duration,
                    qty: qty.value() as f32,
                    is_bid: false,
                });
        }

        self.latest_depth_time = time;
    }

    /// Add a trade to the heatmap
    ///
    /// This aggregates trades into time buckets and groups them by price level.
    fn add_trade(&mut self, trade: &DomainTrade, basis: ChartBasis, tick_size: DataPrice) {
        let time = trade.time.to_millis();
        let bucket_time = Self::bucket_time(time, basis);

        let entry = self
            .trades_by_time
            .entry(bucket_time)
            .or_insert_with(TradeDataPoint::default);

        let qty = trade.quantity.value() as f32;
        let is_sell = trade.is_sell();

        // Accumulate volume
        if is_sell {
            entry.sell_volume += qty;
        } else {
            entry.buy_volume += qty;
        }

        // Group trades at the same price level
        let price_rounded = trade.price.round_to_step(tick_size);
        if let Some(existing) = entry
            .grouped_trades
            .iter_mut()
            .find(|t| t.price == price_rounded && t.is_sell == is_sell)
        {
            existing.qty += qty;
        } else {
            entry.grouped_trades.push(GroupedTrade {
                price: price_rounded,
                qty,
                is_sell,
            });
        }
    }

    /// Calculate bucket time based on chart basis
    fn bucket_time(time: u64, basis: ChartBasis) -> u64 {
        match basis {
            ChartBasis::Time(timeframe) => {
                let interval = timeframe.to_millis();
                (time / interval) * interval
            }
            ChartBasis::Tick(_) => time, // For tick basis, use exact time
        }
    }

    /// Get bucket duration in milliseconds
    fn bucket_duration(basis: ChartBasis) -> u64 {
        match basis {
            ChartBasis::Time(timeframe) => timeframe.to_millis(),
            ChartBasis::Tick(_) => 1000, // 1 second default for tick basis
        }
    }

    /// Iterate over depth runs in a time/price range
    fn iter_depth_filtered(
        &self,
        earliest: u64,
        latest: u64,
        highest: DataPrice,
        lowest: DataPrice,
    ) -> impl Iterator<Item = (&i64, &Vec<DepthRun>)> {
        let highest_units = highest.to_units();
        let lowest_units = lowest.to_units();

        self.depth_by_price
            .range(lowest_units..=highest_units)
            .filter(move |(_, runs)| {
                runs.iter()
                    .any(|run| run.until_time > earliest && run.start_time < latest)
            })
    }

    /// Get the latest orderbook snapshot at a specific time
    ///
    /// This returns the most recent depth run for each price level.
    fn latest_order_runs(
        &self,
        highest: DataPrice,
        lowest: DataPrice,
        latest_time: u64,
    ) -> impl Iterator<Item = (DataPrice, &DepthRun)> {
        let highest_units = highest.to_units();
        let lowest_units = lowest.to_units();

        self.depth_by_price
            .range(lowest_units..=highest_units)
            .filter_map(move |(price_units, runs)| {
                runs.iter()
                    .filter(|run| run.start_time <= latest_time && run.until_time >= latest_time)
                    .max_by_key(|run| run.start_time)
                    .map(|run| (DataPrice::from_units(*price_units), run))
            })
    }

    /// Calculate maximum depth quantity in range (for color scaling)
    fn max_depth_qty_in_range(
        &self,
        earliest: u64,
        latest: u64,
        highest: DataPrice,
        lowest: DataPrice,
        min_qty_filter: f32,
    ) -> f32 {
        self.iter_depth_filtered(earliest, latest, highest, lowest)
            .flat_map(|(_, runs)| runs.iter())
            .filter(|run| run.qty > min_qty_filter)
            .map(|run| run.qty)
            .fold(f32::MIN, f32::max)
            .max(1.0)
    }

    /// Calculate maximum trade quantity in range (for circle sizing)
    fn max_trade_qty_in_range(&self, earliest: u64, latest: u64) -> f32 {
        self.trades_by_time
            .range(earliest..=latest)
            .flat_map(|(_, dp)| dp.grouped_trades.iter())
            .map(|trade| trade.qty)
            .fold(f32::MIN, f32::max)
            .max(1.0)
    }

    /// Calculate maximum aggregate volume in range (for volume bar scaling)
    fn max_aggr_volume_in_range(&self, earliest: u64, latest: u64) -> f32 {
        self.trades_by_time
            .range(earliest..=latest)
            .map(|(_, dp)| dp.buy_volume + dp.sell_volume)
            .fold(f32::MIN, f32::max)
            .max(1.0)
    }
}

/// Quantity scales for rendering
struct QtyScale {
    max_trade_qty: f32,
    max_aggr_volume: f32,
    max_depth_qty: f32,
}

// =============================================================================
// Main Chart Structure
// =============================================================================

/// Heatmap Chart - Volume Heatmap with Order Flow
///
/// This chart visualizes orderbook depth over time as a colored heatmap,
/// with trade executions overlaid as circles.
///
/// ## Architecture
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────┐
/// │ HeatmapChart                                                │
/// │                                                             │
/// │  ┌──────────────┐    ┌─────────────────────────────────┐  │
/// │  │  ChartData   │───▶│      HeatmapData                │  │
/// │  │              │    │                                 │  │
/// │  │ - trades     │    │  - depth_by_price (BTreeMap)   │  │
/// │  │ - depth      │    │  - trades_by_time (BTreeMap)   │  │
/// │  └──────────────┘    └─────────────────────────────────┘  │
/// │         │                         │                        │
/// │         ▼                         ▼                        │
/// │    Rendering Pipeline      Rendering Pipeline             │
/// │    (Trade Circles)         (Colored Heatmap)              │
/// └─────────────────────────────────────────────────────────────┘
/// ```
pub struct HeatmapChart {
    /// View state (camera, bounds, caching)
    chart: ViewState,

    /// Source data (trades + depth snapshots)
    chart_data: ChartData,

    /// Chart basis (time or tick aggregation)
    basis: ChartBasis,

    /// Ticker information (tick size, decimals, etc.)
    ticker_info: FuturesTickerInfo,

    /// Processed heatmap data (depth runs + trade buckets)
    heatmap_data: HeatmapData,

    /// Active indicators
    indicators: EnumMap<HeatmapIndicator, Option<IndicatorData>>,

    /// Visual configuration (filters, scaling)
    visual_config: VisualConfig,

    /// Study configurator UI state
    study_configurator: study::Configurator<HeatmapStudy>,

    /// Active studies
    pub studies: Vec<HeatmapStudy>,

    /// Last update timestamp (for cache invalidation)
    last_tick: Instant,
}

impl HeatmapChart {
    /// Create a new heatmap chart from chart data
    ///
    /// This is the primary constructor. It takes historical data and builds
    /// the heatmap data structure for efficient rendering.
    ///
    /// # Arguments
    ///
    /// * `chart_data` - Historical trades and depth snapshots
    /// * `basis` - Time or tick aggregation basis
    /// * `ticker_info` - Ticker metadata (tick size, etc.)
    /// * `layout` - View configuration (splits, autoscale)
    /// * `indicators` - Active indicators to display
    /// * `studies` - Active studies (volume profile, etc.)
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
            indicators_map[indicator] = Some(match indicator {
                HeatmapIndicator::Volume => IndicatorData::Volume,
                _ => IndicatorData::Volume,
            });
        }

        // Build heatmap data from chart_data
        let mut heatmap_data = HeatmapData::new();

        // Process depth snapshots if available
        if let Some(depth_snapshots) = &chart_data.depth_snapshots {
            for snapshot in depth_snapshots {
                heatmap_data.add_depth_snapshot(snapshot, basis, tick_size);
            }
        }

        // Process trades
        for trade in &chart_data.trades {
            heatmap_data.add_trade(trade, basis, tick_size);
        }

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
            ticker_info.clone(),
            ViewConfig {
                splits: layout.splits,
                autoscale: Some(data::Autoscale::CenterLatest),
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
        };

        // Set initial price and position
        chart.chart.base_price_y = exchange::util::Price::from_units(base_price.to_units());
        chart.chart.latest_x = latest_x;
        chart.chart.last_price = Some(PriceInfoLabel::Neutral(exchange::util::Price::from_units(base_price.to_units())));

        chart
    }

    /// Update heatmap from replay mode
    ///
    /// This adds new depth snapshots and trades to the existing data,
    /// allowing for dynamic updates during replay.
    ///
    /// # Arguments
    ///
    /// * `depth` - New depth snapshot
    /// * `trades` - New trades to add
    pub fn update_from_replay(&mut self, depth: &DepthSnapshot, trades: &[DomainTrade]) {
        let tick_size = DataPrice::from_f32(self.ticker_info.tick_size);

        // Add new depth snapshot
        self.heatmap_data
            .add_depth_snapshot(depth, self.basis, tick_size);

        // Add new trades
        for trade in trades {
            self.heatmap_data.add_trade(trade, self.basis, tick_size);
        }

        // Update chart state
        if let Some(mid_price) = depth.mid_price() {
            self.chart.base_price_y = exchange::util::Price::from_units(mid_price.to_units());
            self.chart.last_price = Some(PriceInfoLabel::Neutral(exchange::util::Price::from_units(mid_price.to_units())));
        }

        self.chart.latest_x = depth.time.to_millis();
        self.invalidate(Some(Instant::now()));
    }

    /// Get current visual configuration
    pub fn visual_config(&self) -> VisualConfig {
        self.visual_config
    }

    /// Set visual configuration
    ///
    /// This updates filters and visual settings for the heatmap.
    pub fn set_visual_config(&mut self, visual_config: VisualConfig) {
        self.visual_config = visual_config;
        self.invalidate(Some(Instant::now()));
    }

    /// Change chart basis (time or tick aggregation)
    ///
    /// This rebuilds the entire heatmap data structure with the new basis.
    pub fn set_basis(&mut self, basis: ChartBasis) {
        self.basis = basis;
        self.chart.basis = basis;

        // Rebuild heatmap data with new basis
        let tick_size = DataPrice::from_f32(self.ticker_info.tick_size);
        let mut heatmap_data = HeatmapData::new();

        if let Some(depth_snapshots) = &self.chart_data.depth_snapshots {
            for snapshot in depth_snapshots {
                heatmap_data.add_depth_snapshot(snapshot, basis, tick_size);
            }
        }

        for trade in &self.chart_data.trades {
            heatmap_data.add_trade(trade, basis, tick_size);
        }

        self.heatmap_data = heatmap_data;

        // Reset translation
        let chart = &mut self.chart;
        chart.translation = Vector::new(
            0.5 * (chart.bounds.width / chart.scaling) - (90.0 / chart.scaling),
            0.0,
        );

        self.invalidate(None);
    }

    /// Get study configurator
    pub fn study_configurator(&self) -> &study::Configurator<HeatmapStudy> {
        &self.study_configurator
    }

    /// Update study configurator (add/remove/configure studies)
    pub fn update_study_configurator(&mut self, message: study::Message<HeatmapStudy>) {
        let studies = &mut self.studies;

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

        self.invalidate(None);
    }

    /// Get basis interval in milliseconds (None for tick basis)
    pub fn basis_interval(&self) -> Option<u64> {
        match self.basis {
            ChartBasis::Time(timeframe) => Some(timeframe.to_millis()),
            ChartBasis::Tick(_) => None,
        }
    }

    /// Get chart layout configuration
    pub fn chart_layout(&self) -> ViewConfig {
        self.chart.layout()
    }

    /// Change tick size (price aggregation)
    ///
    /// This rebuilds the heatmap with a different tick size for price levels.
    pub fn change_tick_size(&mut self, new_tick_size: f32) {
        let chart_state = self.mut_state();

        let step = exchange::util::PriceStep::from_f32(new_tick_size);

        chart_state.cell_height = 4.0;
        chart_state.tick_size = step;
        chart_state.decimals = count_decimals(new_tick_size);

        // Rebuild heatmap with new tick size
        let tick_size = DataPrice::from_f32(new_tick_size);
        let mut heatmap_data = HeatmapData::new();

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

    /// Toggle indicator on/off
    pub fn toggle_indicator(&mut self, indicator: HeatmapIndicator) {
        if self.indicators[indicator].is_some() {
            self.indicators[indicator] = None;
        } else {
            let data = match indicator {
                HeatmapIndicator::Volume => IndicatorData::Volume,
                _ => IndicatorData::Volume,
            };
            self.indicators[indicator] = Some(data);
        }
    }

    /// Invalidate caches and trigger redraw
    pub fn invalidate(&mut self, now: Option<Instant>) -> Option<super::Action> {
        let chart = &mut self.chart;

        // Apply autoscale if enabled
        if chart.layout.autoscale.is_some() {
            chart.translation = Vector::new(
                0.5 * (chart.bounds.width / chart.scaling) - (90.0 / chart.scaling),
                0.0,
            );
        }

        chart.cache.clear_all();

        if let Some(t) = now {
            self.last_tick = t;
        }

        None
    }

    /// Get last update time
    pub fn last_update(&self) -> Instant {
        self.last_tick
    }

    /// Calculate quantity scales for rendering
    ///
    /// This determines the maximum quantities in the visible range
    /// for color/size scaling.
    fn calc_qty_scales(
        &self,
        earliest: u64,
        latest: u64,
        highest: DataPrice,
        lowest: DataPrice,
    ) -> QtyScale {
        let max_trade_qty = self
            .heatmap_data
            .max_trade_qty_in_range(earliest, latest);

        let max_aggr_volume = self
            .heatmap_data
            .max_aggr_volume_in_range(earliest, latest);

        let max_depth_qty = self.heatmap_data.max_depth_qty_in_range(
            earliest,
            latest,
            highest,
            lowest,
            self.visual_config.order_size_filter,
        );

        QtyScale {
            max_trade_qty,
            max_aggr_volume,
            max_depth_qty,
        }
    }
}

// =============================================================================
// Canvas Rendering
// =============================================================================

impl canvas::Program<Message> for HeatmapChart {
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

        // Main heatmap layer
        let heatmap = chart.cache.main.draw(renderer, bounds_size, |frame| {
            let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

            frame.translate(center);
            frame.scale(chart.scaling);
            frame.translate(chart.translation);

            let region = chart.visible_region(frame.size());

            let (earliest, latest) = chart.interval_range(&region);
            let (highest_exch, lowest_exch) = chart.price_range(&region);

            // Convert exchange::util::Price to data::Price
            let highest = DataPrice::from_units(highest_exch.units);
            let lowest = DataPrice::from_units(lowest_exch.units);

            if latest < earliest {
                return;
            }

            let cell_height = chart.cell_height;
            let qty_scales = self.calc_qty_scales(earliest, latest, highest, lowest);

            let max_depth_qty = qty_scales.max_depth_qty;
            let (max_aggr_volume, max_trade_qty) =
                (qty_scales.max_aggr_volume, qty_scales.max_trade_qty);

            let volume_indicator = self.indicators[HeatmapIndicator::Volume].is_some();

            // =========================================================
            // Draw Depth Heatmap
            // =========================================================
            for (price_units, runs) in
                self.heatmap_data
                    .iter_depth_filtered(earliest, latest, highest, lowest)
            {
                let price = DataPrice::from_units(*price_units);
                let y_position = chart.price_to_y(exchange::util::Price::from_units(price.to_units()));

                for run in runs.iter() {
                    if run.qty <= self.visual_config.order_size_filter {
                        continue;
                    }

                    let start_x = chart.interval_to_x(run.start_time.max(earliest));
                    let end_x = chart.interval_to_x(run.until_time.min(latest)).min(0.0);

                    let width = end_x - start_x;

                    if width > 0.001 {
                        let color_alpha = (run.qty / max_depth_qty).min(1.0);

                        frame.fill_rectangle(
                            Point::new(start_x, y_position - (cell_height / 2.0)),
                            Size::new(width, cell_height),
                            depth_color(palette, run.is_bid, color_alpha),
                        );
                    }
                }
            }

            // =========================================================
            // Draw Latest Orderbook Bars
            // =========================================================
            if !self.heatmap_data.trades_by_time.is_empty() {
                let latest_timestamp = self.heatmap_data.latest_depth_time;
                let latest_runs: Vec<_> = self
                    .heatmap_data
                    .latest_order_runs(highest, lowest, latest_timestamp)
                    .collect();

                let max_qty = latest_runs
                    .iter()
                    .map(|(_, run)| run.qty())
                    .fold(f32::MIN, f32::max)
                    .ceil()
                    * 5.0
                    / 5.0;

                if !max_qty.is_infinite() && max_qty > 0.0 {
                    // Draw bars
                    for (price, run) in latest_runs {
                        let y_position = chart.price_to_y(exchange::util::Price::from_units(price.to_units()));
                        let bar_width = (run.qty() / max_qty) * 50.0;

                        frame.fill_rectangle(
                            Point::new(0.0, y_position - (cell_height / 2.0)),
                            Size::new(bar_width, cell_height),
                            depth_color(palette, run.is_bid, 0.5),
                        );
                    }

                    // Draw max quantity label
                    let text_size = 9.0 / chart.scaling;
                    let text_content = abbr_large_numbers(max_qty);
                    let text_position = Point::new(50.0, region.y);

                    frame.fill_text(canvas::Text {
                        content: text_content,
                        position: text_position,
                        size: iced::Pixels(text_size),
                        color: palette.background.base.text,
                        font: style::AZERET_MONO,
                        ..canvas::Text::default()
                    });
                }
            }

            // =========================================================
            // Draw Trade Markers
            // =========================================================
            for (time, dp) in self.heatmap_data.trades_by_time.range(earliest..=latest) {
                let x_position = chart.interval_to_x(*time);

                for trade in &dp.grouped_trades {
                    let y_position = chart.price_to_y(exchange::util::Price::from_units(trade.price.to_units()));

                    if trade.qty > self.visual_config.trade_size_filter {
                        let color = if trade.is_sell {
                            palette.danger.base.color
                        } else {
                            palette.success.base.color
                        };

                        let radius = {
                            if let Some(trade_size_scale) = self.visual_config.trade_size_scale {
                                let scale_factor = (trade_size_scale as f32) / 100.0;
                                1.0 + (trade.qty / max_trade_qty)
                                    * (MAX_CIRCLE_RADIUS - 1.0)
                                    * scale_factor
                            } else {
                                cell_height / 2.0
                            }
                        };

                        frame.fill(
                            &Path::circle(Point::new(x_position, y_position), radius),
                            color,
                        );
                    }
                }

                // =========================================================
                // Draw Volume Indicator
                // =========================================================
                if volume_indicator {
                    let bar_width = (chart.cell_width / 2.0) * 0.9;
                    let area_height = (bounds.height / chart.scaling) * 0.1;

                    let (buy_volume, sell_volume) = (dp.buy_volume, dp.sell_volume);

                    super::draw_volume_bar(
                        frame,
                        x_position,
                        (region.y + region.height) - area_height,
                        buy_volume,
                        sell_volume,
                        max_aggr_volume,
                        area_height,
                        bar_width,
                        palette.success.base.color,
                        palette.danger.base.color,
                        1.0,
                        false,
                    );
                }
            }

            // Draw max volume label
            if volume_indicator && max_aggr_volume > 0.0 {
                let text_size = 9.0 / chart.scaling;
                let text_content = abbr_large_numbers(max_aggr_volume);
                let text_width = (text_content.len() as f32 * text_size) / 1.5;

                let text_position = Point::new(
                    (region.x + region.width) - text_width,
                    (region.y + region.height) - (bounds.height / chart.scaling) * 0.1 - text_size,
                );

                frame.fill_text(canvas::Text {
                    content: text_content,
                    position: text_position,
                    size: text_size.into(),
                    color: palette.background.base.text,
                    font: style::AZERET_MONO,
                    ..canvas::Text::default()
                });
            }

            // =========================================================
            // Draw Volume Profile Study
            // =========================================================
            let volume_profile: Option<&ProfileKind> = self
                .studies
                .iter()
                .find_map(|study| match study {
                    HeatmapStudy::VolumeProfile(profile) => Some(profile),
                });

            if let Some(profile_kind) = volume_profile {
                let area_width = (bounds.width / chart.scaling) * 0.1;

                let min_segment_width = 2.0;
                let segments = ((area_width / min_segment_width).floor() as usize).clamp(10, 40);

                // Draw background gradient
                for i in 0..segments {
                    let segment_width = area_width / segments as f32;
                    let segment_x = region.x + (i as f32 * segment_width);

                    let alpha = 0.95 - (0.85 * (i as f32 / (segments - 1) as f32).powf(2.0));

                    frame.fill_rectangle(
                        Point::new(segment_x, region.y),
                        Size::new(segment_width, region.height),
                        palette.background.weakest.color.scale_alpha(alpha),
                    );
                }

                // Draw volume profile
                draw_volume_profile(
                    frame,
                    &region,
                    profile_kind,
                    palette,
                    chart,
                    &self.heatmap_data,
                    area_width,
                    self.basis,
                );
            }
        });

        // Crosshair layer
        if !self.is_empty() {
            let crosshair = chart.cache.crosshair.draw(renderer, bounds_size, |frame| {
                if let Some(cursor_position) = cursor.position_in(bounds) {
                    let (_cursor_at_price, _cursor_at_time) = chart.draw_crosshair(
                        frame,
                        theme,
                        bounds_size,
                        cursor_position,
                        interaction,
                    );

                    // Skip tooltip during interactions
                    if matches!(interaction, Interaction::Panning { .. })
                        || matches!(interaction, Interaction::Ruler { start } if start.is_some())
                    {
                        return;
                    }

                    // TODO: Draw depth tooltip at cursor position
                }
            });

            vec![heatmap, crosshair]
        } else {
            vec![heatmap]
        }
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
                    return mouse::Interaction::Crosshair;
                }
                mouse::Interaction::default()
            }
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Get depth color based on side and alpha
fn depth_color(palette: &Extended, is_bid: bool, alpha: f32) -> Color {
    if is_bid {
        palette.success.strong.color.scale_alpha(alpha)
    } else {
        palette.danger.strong.color.scale_alpha(alpha)
    }
}

/// Draw volume profile on the left side of the chart
fn draw_volume_profile(
    frame: &mut canvas::Frame,
    region: &Rectangle,
    kind: &ProfileKind,
    palette: &Extended,
    chart: &ViewState,
    heatmap_data: &HeatmapData,
    area_width: f32,
    basis: ChartBasis,
) {
    let (highest_exch, lowest_exch) = chart.price_range(region);

    // Convert to data::Price
    let highest = DataPrice::from_units(highest_exch.units);
    let lowest = DataPrice::from_units(lowest_exch.units);

    // Calculate time range based on profile kind
    let time_range = match kind {
        ProfileKind::VisibleRange => {
            let earliest = chart.x_to_interval(region.x);
            let latest = chart.x_to_interval(region.x + region.width);
            earliest..=latest
        }
        ProfileKind::FixedWindow { candles: datapoints } | ProfileKind::Fixed(datapoints) => {
            let basis_interval: u64 = match basis {
                ChartBasis::Time(timeframe) => timeframe.to_millis(),
                ChartBasis::Tick(_) => return,
            };

            let latest = chart
                .latest_x
                .min(chart.x_to_interval(region.x + region.width));
            let earliest = latest.saturating_sub((*datapoints as u64) * basis_interval);

            earliest..=latest
        }
    };

    let step = chart.tick_size;
    let step_as_price = DataPrice::from_units(step.units);

    let first_tick = lowest.round_to_side_step(false, step_as_price);
    let last_tick = highest.round_to_side_step(true, step_as_price);

    let num_ticks = match exchange::util::Price::steps_between_inclusive(
        exchange::util::Price::from_units(first_tick.to_units()),
        exchange::util::Price::from_units(last_tick.to_units()),
        step,
    ) {
        Some(n) => n,
        None => return,
    };

    if num_ticks > 4096 {
        return;
    }

    // Build volume profile
    let mut profile = vec![(0.0f32, 0.0f32); num_ticks];
    let mut max_aggr_volume = 0.0f32;

    heatmap_data
        .trades_by_time
        .range(time_range)
        .for_each(|(_, dp)| {
            dp.grouped_trades
                .iter()
                .filter(|trade| {
                    let trade_price: DataPrice = trade.price;
                    trade_price >= lowest && trade_price <= highest
                })
                .for_each(|trade| {
                    let grouped_price = if trade.is_sell {
                        trade.price.round_to_side_step(true, step_as_price)
                    } else {
                        trade.price.round_to_side_step(false, step_as_price)
                    };

                    let first_tick_price: DataPrice = first_tick;
                    let last_tick_price: DataPrice = last_tick;
                    let grouped_price_units = grouped_price.to_units();

                    if grouped_price_units < first_tick_price.to_units()
                        || grouped_price_units > last_tick_price.to_units()
                    {
                        return;
                    }

                    let index = ((grouped_price_units - first_tick_price.to_units())
                        / step.units as i64) as usize;

                    if let Some(entry) = profile.get_mut(index) {
                        if trade.is_sell {
                            entry.1 += trade.qty;
                        } else {
                            entry.0 += trade.qty;
                        }
                        max_aggr_volume = max_aggr_volume.max(entry.0 + entry.1);
                    }
                });
        });

    // Draw volume bars
    profile
        .iter()
        .enumerate()
        .for_each(|(index, (buy_v, sell_v))| {
            if *buy_v > 0.0 || *sell_v > 0.0 {
                let price: DataPrice = first_tick;
                let price = price.add_steps(index as i64, step_as_price);
                let y_position = chart.price_to_y(exchange::util::Price::from_units(price.to_units()));

                let next_price = price.add_steps(1, step_as_price);
                let next_y_position = chart.price_to_y(exchange::util::Price::from_units(next_price.to_units()));
                let bar_height = (next_y_position - y_position).abs();

                super::draw_volume_bar(
                    frame,
                    region.x,
                    y_position,
                    *buy_v,
                    *sell_v,
                    max_aggr_volume,
                    area_width,
                    bar_height,
                    palette.success.weak.color,
                    palette.danger.weak.color,
                    1.0,
                    true,
                );
            }
        });

    // Draw max volume label
    if max_aggr_volume > 0.0 {
        let text_size = 9.0 / chart.scaling;
        let text_content = abbr_large_numbers(max_aggr_volume);

        let text_position = Point::new(region.x + area_width, region.y);

        frame.fill_text(canvas::Text {
            content: text_content,
            position: text_position,
            size: iced::Pixels(text_size),
            color: palette.background.base.text,
            font: style::AZERET_MONO,
            ..canvas::Text::default()
        });
    }
}

