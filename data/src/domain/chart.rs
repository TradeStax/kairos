//! Chart Domain Model
//!
//! Domain types for chart configuration, data, and UI bridge types.

use super::entities::{Candle, DepthSnapshot, Trade};
use super::types::{DateRange, TimeRange, Timestamp};
use crate::domain::{FuturesTicker, Timeframe};
use crate::feed::FeedId;
use serde::{Deserialize, Serialize};

/// Chart configuration (what to display)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartConfig {
    /// Ticker to display
    pub ticker: FuturesTicker,
    /// Timeframe (M1, M5, H1, etc.) or tick count
    pub basis: ChartBasis,
    /// Date range to load
    pub date_range: DateRange,
    /// Chart type
    pub chart_type: ChartType,
}

/// Chart basis (time-based or tick-based)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChartBasis {
    /// Time-based (M1, M5, H1, etc.)
    Time(Timeframe),
    /// Tick-based (50T, 100T, etc.)
    Tick(u32),
}

impl ChartBasis {
    pub fn is_time(&self) -> bool {
        matches!(self, ChartBasis::Time(_))
    }

    pub fn is_tick(&self) -> bool {
        matches!(self, ChartBasis::Tick(_))
    }

    pub fn timeframe(&self) -> Option<Timeframe> {
        match self {
            ChartBasis::Time(tf) => Some(*tf),
            ChartBasis::Tick(_) => None,
        }
    }

    pub fn tick_count(&self) -> Option<u32> {
        match self {
            ChartBasis::Time(_) => None,
            ChartBasis::Tick(count) => Some(*count),
        }
    }
}

impl std::fmt::Display for ChartBasis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChartBasis::Time(tf) => write!(f, "{:?}", tf),
            ChartBasis::Tick(count) => write!(f, "{}T", count),
        }
    }
}

impl From<Timeframe> for ChartBasis {
    fn from(tf: Timeframe) -> Self {
        ChartBasis::Time(tf)
    }
}

/// Chart type (candlestick, heatmap, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartType {
    /// Standard candlestick chart (also hosts footprint studies)
    Candlestick,
    /// Line chart
    Line,
    /// Heikin-Ashi candlestick chart
    HeikinAshi,
    /// Heatmap (orderbook visualization)
    Heatmap,
}

impl std::fmt::Display for ChartType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChartType::Candlestick => write!(f, "Candlestick"),
            ChartType::Line => write!(f, "Line"),
            ChartType::HeikinAshi => write!(f, "Heikin-Ashi"),
            ChartType::Heatmap => write!(f, "Heatmap"),
        }
    }
}

/// Kind of data gap detected during multi-feed merging
#[derive(Debug, Clone, PartialEq)]
pub enum DataGapKind {
    /// No data available from any feed for this period
    NoData,
    /// Market was closed (weekend, holiday, outside trading hours)
    MarketClosed,
    /// Only partial coverage from some feeds
    PartialCoverage {
        /// Which feeds had data for this period
        available_feeds: Vec<FeedId>,
    },
}

/// A gap in the data timeline
#[derive(Debug, Clone, PartialEq)]
pub struct DataGap {
    /// Start timestamp (ms since epoch)
    pub start: Timestamp,
    /// End timestamp (ms since epoch)
    pub end: Timestamp,
    /// What kind of gap this is
    pub kind: DataGapKind,
}

impl DataGap {
    pub fn new(start: Timestamp, end: Timestamp, kind: DataGapKind) -> Self {
        Self { start, end, kind }
    }

    /// Duration of this gap in milliseconds
    pub fn duration_ms(&self) -> u64 {
        self.end.0.saturating_sub(self.start.0)
    }
}

/// A segment of data from a specific feed
#[derive(Debug, Clone)]
pub struct DataSegment {
    /// Which feed provided this data
    pub feed_id: FeedId,
    /// Start timestamp
    pub start: Timestamp,
    /// End timestamp
    pub end: Timestamp,
    /// Trades in this segment
    pub trades: Vec<Trade>,
}

/// Result of merging data from multiple feeds
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// Merged and deduplicated trades, sorted by time
    pub trades: Vec<Trade>,
    /// Gaps detected in the merged data
    pub gaps: Vec<DataGap>,
    /// Which feeds contributed data
    pub feed_ids: Vec<FeedId>,
}

/// Chart data (actual market data)
///
/// This is the in-memory representation of all chart data.
/// Trades are ALWAYS kept in memory to enable instant basis switching.
#[derive(Debug, Clone)]
pub struct ChartData {
    /// Raw tick-by-tick trades (PRIMARY SOURCE OF TRUTH)
    pub trades: Vec<Trade>,

    /// Aggregated candles (DERIVED from trades)
    /// These are built locally from trades, never fetched separately
    pub candles: Vec<Candle>,

    /// Depth snapshots (OPTIONAL, only for heatmap)
    /// This is a separate data source from trades
    pub depth_snapshots: Option<Vec<DepthSnapshot>>,

    /// Data gaps detected during multi-feed merging
    pub gaps: Vec<DataGap>,

    /// Time range of the data
    pub time_range: TimeRange,
}

impl ChartData {
    /// Create new chart data from trades
    pub fn from_trades(trades: Vec<Trade>, candles: Vec<Candle>) -> Self {
        let time_range = if let (Some(first), Some(last)) = (trades.first(), trades.last()) {
            TimeRange::new(first.time, last.time)
        } else {
            TimeRange::new(super::types::Timestamp(0), super::types::Timestamp(0))
        };

        Self {
            trades,
            candles,
            depth_snapshots: None,
            gaps: Vec::new(),
            time_range,
        }
    }

    /// Create chart data with depth snapshots
    pub fn with_depth(mut self, depth_snapshots: Vec<DepthSnapshot>) -> Self {
        self.depth_snapshots = Some(depth_snapshots);
        self
    }

    /// Create chart data with gap information
    pub fn with_gaps(mut self, gaps: Vec<DataGap>) -> Self {
        self.gaps = gaps;
        self
    }

    /// Check if there are any data gaps
    pub fn has_gaps(&self) -> bool {
        !self.gaps.is_empty()
    }

    /// Check if trades are loaded
    pub fn has_trades(&self) -> bool {
        !self.trades.is_empty()
    }

    /// Check if candles are loaded
    pub fn has_candles(&self) -> bool {
        !self.candles.is_empty()
    }

    /// Check if depth data is loaded
    pub fn has_depth(&self) -> bool {
        self.depth_snapshots.as_ref().is_some_and(|d| !d.is_empty())
    }

    /// Get number of trades
    pub fn trade_count(&self) -> usize {
        self.trades.len()
    }

    /// Get number of candles
    pub fn candle_count(&self) -> usize {
        self.candles.len()
    }

    /// Get memory usage estimate (bytes)
    pub fn memory_usage(&self) -> usize {
        let trade_size = std::mem::size_of::<Trade>();
        let candle_size = std::mem::size_of::<Candle>();

        let trades_mem = self.trades.len() * trade_size;
        let candles_mem = self.candles.len() * candle_size;

        // Depth snapshots are variable size, rough estimate
        let depth_mem = self.depth_snapshots.as_ref().map_or(0, |d| d.len() * 1024); // ~1KB per snapshot

        let gaps_mem = self.gaps.len() * std::mem::size_of::<DataGap>();

        trades_mem + candles_mem + depth_mem + gaps_mem
    }
}

/// Loading status for chart data
#[derive(Debug, Clone, PartialEq)]
pub enum LoadingStatus {
    /// Idle, no loading in progress
    Idle,

    /// Downloading from exchange
    Downloading {
        schema: DataSchema,
        days_total: usize,
        days_complete: usize,
        current_day: String,
    },

    /// Loading from local cache
    LoadingFromCache {
        schema: DataSchema,
        days_total: usize,
        days_loaded: usize,
        items_loaded: usize,
    },

    /// Building chart (aggregating, processing)
    Building {
        operation: String,
        progress: f32, // 0.0 to 1.0
    },

    /// Ready to display
    Ready,

    /// Error occurred
    Error { message: String },
}

impl LoadingStatus {
    pub fn is_loading(&self) -> bool {
        matches!(
            self,
            LoadingStatus::Downloading { .. }
                | LoadingStatus::LoadingFromCache { .. }
                | LoadingStatus::Building { .. }
        )
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, LoadingStatus::Ready)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, LoadingStatus::Error { .. })
    }
}

/// Data schema being loaded
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSchema {
    Trades,
    MBP10,
    OHLCV,
    Options,
}

impl std::fmt::Display for DataSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSchema::Trades => write!(f, "Trades"),
            DataSchema::MBP10 => write!(f, "MBP-10"),
            DataSchema::OHLCV => write!(f, "OHLCV"),
            DataSchema::Options => write!(f, "Options"),
        }
    }
}

// ── Chart UI types (view/rendering bridge) ────────────────────────────

/// View configuration for chart layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewConfig {
    pub splits: Vec<f32>,
    pub autoscale: Option<Autoscale>,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            splits: vec![],
            autoscale: Some(Autoscale::CenterLatest),
        }
    }
}

/// Autoscale mode for charts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Autoscale {
    CenterLatest,
    FitAll,
    Disabled,
}

/// Display mode for footprint studies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FootprintMode {
    /// Grid cells with values at each price level
    Box,
    /// Horizontal bars extending from candle
    #[default]
    Profile,
}

impl std::fmt::Display for FootprintMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintMode::Box => write!(f, "Box"),
            FootprintMode::Profile => write!(f, "Profile"),
        }
    }
}

/// Data type shown in the footprint study
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FootprintType {
    /// Total volume (buy + sell) at each price level
    #[default]
    Volume,
    /// Separate ask and bid volumes on each side
    BidAskSplit,
    /// Buy - Sell difference per price level
    Delta,
    /// Combined: total volume as bar size + delta as adjacent colored sub-bars
    DeltaAndVolume,
}

impl FootprintType {
    pub const ALL: &'static [FootprintType] = &[
        FootprintType::Volume,
        FootprintType::BidAskSplit,
        FootprintType::Delta,
        FootprintType::DeltaAndVolume,
    ];
}

impl std::fmt::Display for FootprintType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintType::Volume => write!(f, "Volume"),
            FootprintType::BidAskSplit => write!(f, "Bid/Ask Split"),
            FootprintType::Delta => write!(f, "Delta"),
            FootprintType::DeltaAndVolume => write!(f, "Delta + Volume"),
        }
    }
}

/// Candle position relative to the study bars
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CandlePosition {
    /// No candle drawn, study bars only
    None,
    /// Candle on left, bars extend right
    #[default]
    Left,
    /// Candle centered, bars on both sides
    Center,
    /// Candle on right, bars extend left
    Right,
}

impl CandlePosition {
    pub const ALL: &'static [CandlePosition] = &[
        CandlePosition::None,
        CandlePosition::Left,
        CandlePosition::Center,
        CandlePosition::Right,
    ];
}

impl std::fmt::Display for CandlePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CandlePosition::None => write!(f, "None"),
            CandlePosition::Left => write!(f, "Left"),
            CandlePosition::Center => write!(f, "Center"),
            CandlePosition::Right => write!(f, "Right"),
        }
    }
}

/// Active footprint study configuration (None = standard candles)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FootprintStudyConfig {
    pub mode: FootprintMode,
    pub study_type: FootprintType,
    pub scaling: ClusterScaling,
    pub candle_position: CandlePosition,
}

impl Default for FootprintStudyConfig {
    fn default() -> Self {
        Self {
            mode: FootprintMode::Profile,
            study_type: FootprintType::Volume,
            scaling: ClusterScaling::Sqrt,
            candle_position: CandlePosition::Left,
        }
    }
}

/// Cluster scaling for footprint charts
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ClusterScaling {
    Linear,
    #[default]
    Sqrt,
    Log,
    VisibleRange,
    Datapoint,
    Hybrid {
        weight: f32,
    },
}

// SAFETY: Manual Eq is sound here because f32 `weight` in Hybrid variant
// is always a finite value (0.0..=1.0 percentage). NaN is never constructed.
impl Eq for ClusterScaling {}

impl ClusterScaling {
    pub const ALL: &'static [ClusterScaling] = &[
        ClusterScaling::Linear,
        ClusterScaling::Sqrt,
        ClusterScaling::Log,
        ClusterScaling::VisibleRange,
        ClusterScaling::Datapoint,
        ClusterScaling::Hybrid { weight: 0.5 },
    ];
}

impl std::fmt::Display for ClusterScaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClusterScaling::Linear => write!(f, "Linear"),
            ClusterScaling::Sqrt => write!(f, "Square Root"),
            ClusterScaling::Log => write!(f, "Logarithmic"),
            ClusterScaling::VisibleRange => write!(f, "Visible Range"),
            ClusterScaling::Datapoint => write!(f, "Datapoint"),
            ClusterScaling::Hybrid { weight } => write!(f, "Hybrid ({:.1})", weight),
        }
    }
}

/// Kline data point (candle with metadata)
#[derive(Debug, Clone)]
pub struct KlineDataPoint {
    pub kline: Candle,
    pub total_volume: f32,
}

impl KlineDataPoint {
    pub fn from_candle(candle: Candle) -> Self {
        Self {
            total_volume: (candle.buy_volume.0 + candle.sell_volume.0) as f32,
            kline: candle,
        }
    }
}

/// Kline trades (for footprint charts)
#[derive(Debug, Clone, Default)]
pub struct KlineTrades {
    pub trades: Vec<TradeCell>,
}

#[derive(Debug, Clone)]
pub struct TradeCell {
    pub price: i64,
    pub buy_volume: f32,
    pub sell_volume: f32,
}

/// Point of control
#[derive(Debug, Clone, Copy)]
pub struct PointOfControl {
    pub price: i64,
    pub volume: f32,
}

/// Naked point of control
#[derive(Debug, Clone, Copy)]
pub struct NPoc {
    pub price: i64,
    pub time: Timestamp,
}

/// Chart indicator trait
pub trait Indicator: Send + Sync {
    fn name(&self) -> &str;
}

/// Kline indicator types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, enum_map::Enum)]
pub enum KlineIndicator {
    Volume,
    Delta,
    OpenInterest,
    Sma20,
    Sma50,
    Sma200,
    Ema9,
    Ema21,
    Rsi14,
    Macd,
    BollingerBands,
}

impl std::fmt::Display for KlineIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KlineIndicator::Volume => write!(f, "Volume"),
            KlineIndicator::Delta => write!(f, "Delta"),
            KlineIndicator::OpenInterest => write!(f, "Open Interest"),
            KlineIndicator::Sma20 => write!(f, "SMA 20"),
            KlineIndicator::Sma50 => write!(f, "SMA 50"),
            KlineIndicator::Sma200 => write!(f, "SMA 200"),
            KlineIndicator::Ema9 => write!(f, "EMA 9"),
            KlineIndicator::Ema21 => write!(f, "EMA 21"),
            KlineIndicator::Rsi14 => write!(f, "RSI 14"),
            KlineIndicator::Macd => write!(f, "MACD"),
            KlineIndicator::BollingerBands => write!(f, "Bollinger Bands"),
        }
    }
}

impl Indicator for KlineIndicator {
    fn name(&self) -> &str {
        match self {
            KlineIndicator::Volume => "Volume",
            KlineIndicator::Delta => "Delta",
            KlineIndicator::OpenInterest => "Open Interest",
            KlineIndicator::Sma20 => "SMA 20",
            KlineIndicator::Sma50 => "SMA 50",
            KlineIndicator::Sma200 => "SMA 200",
            KlineIndicator::Ema9 => "EMA 9",
            KlineIndicator::Ema21 => "EMA 21",
            KlineIndicator::Rsi14 => "RSI 14",
            KlineIndicator::Macd => "MACD",
            KlineIndicator::BollingerBands => "Bollinger Bands",
        }
    }
}

impl KlineIndicator {
    /// Get all indicators available for a given market
    /// For futures, all indicators are available
    pub fn for_market(_market_type: &str) -> Vec<KlineIndicator> {
        vec![
            KlineIndicator::Volume,
            KlineIndicator::Delta,
            KlineIndicator::OpenInterest,
            KlineIndicator::Sma20,
            KlineIndicator::Sma50,
            KlineIndicator::Sma200,
            KlineIndicator::Ema9,
            KlineIndicator::Ema21,
            KlineIndicator::Rsi14,
            KlineIndicator::Macd,
            KlineIndicator::BollingerBands,
        ]
    }

    /// Get all available indicators
    pub fn all_indicators() -> Vec<KlineIndicator> {
        vec![
            KlineIndicator::Volume,
            KlineIndicator::Delta,
            KlineIndicator::OpenInterest,
            KlineIndicator::Sma20,
            KlineIndicator::Sma50,
            KlineIndicator::Sma200,
            KlineIndicator::Ema9,
            KlineIndicator::Ema21,
            KlineIndicator::Rsi14,
            KlineIndicator::Macd,
            KlineIndicator::BollingerBands,
        ]
    }

    /// Whether this indicator overlays on the main price chart
    /// rather than rendering in a separate panel below.
    ///
    /// Overlay indicators (SMA, EMA, Bollinger Bands) produce values
    /// in price space and are drawn directly on the candlestick chart.
    /// Panel indicators (Volume, Delta, RSI, MACD, OI) have their own
    /// Y-axis scale and are shown in separate sub-panels.
    pub fn is_overlay(&self) -> bool {
        matches!(
            self,
            KlineIndicator::Sma20
                | KlineIndicator::Sma50
                | KlineIndicator::Sma200
                | KlineIndicator::Ema9
                | KlineIndicator::Ema21
                | KlineIndicator::BollingerBands
        )
    }
}

/// Heatmap indicator types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, enum_map::Enum)]
pub enum HeatmapIndicator {
    Volume,
    Delta,
    Trades,
}

impl std::fmt::Display for HeatmapIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeatmapIndicator::Volume => write!(f, "Volume"),
            HeatmapIndicator::Delta => write!(f, "Delta"),
            HeatmapIndicator::Trades => write!(f, "Trades"),
        }
    }
}

impl Indicator for HeatmapIndicator {
    fn name(&self) -> &str {
        match self {
            HeatmapIndicator::Volume => "Volume",
            HeatmapIndicator::Delta => "Delta",
            HeatmapIndicator::Trades => "Trades",
        }
    }
}

impl HeatmapIndicator {
    /// Get all available indicators
    pub fn all_indicators() -> Vec<HeatmapIndicator> {
        vec![
            HeatmapIndicator::Volume,
            HeatmapIndicator::Delta,
            HeatmapIndicator::Trades,
        ]
    }
}

/// Heatmap study types
pub mod heatmap {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum HeatmapStudy {
        VolumeProfile(ProfileKind),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum ProfileKind {
        VisibleRange,
        FixedWindow { candles: usize },
        Fixed(usize), // Alias for FixedWindow
    }

    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub enum CoalesceKind {
        None,
        Adjacent,
        All,
        Average(f32),
        First(f32),
        Max(f32),
    }

    // SAFETY: Manual Eq is sound here because f32 thresholds in Average/First/Max
    // are always finite values set via UI sliders. NaN is never constructed.
    impl Eq for CoalesceKind {}

    impl CoalesceKind {
        pub fn threshold(&self) -> f32 {
            match self {
                CoalesceKind::Average(t) | CoalesceKind::First(t) | CoalesceKind::Max(t) => *t,
                _ => 0.0,
            }
        }

        pub fn with_threshold(&self, threshold: f32) -> Self {
            match self {
                CoalesceKind::Average(_) => CoalesceKind::Average(threshold),
                CoalesceKind::First(_) => CoalesceKind::First(threshold),
                CoalesceKind::Max(_) => CoalesceKind::Max(threshold),
                other => *other,
            }
        }
    }

    impl HeatmapStudy {
        pub const ALL: &'static [HeatmapStudy] =
            &[HeatmapStudy::VolumeProfile(ProfileKind::VisibleRange)];

        pub fn is_same_type(&self, other: &Self) -> bool {
            std::mem::discriminant(self) == std::mem::discriminant(other)
        }
    }

    impl std::fmt::Display for HeatmapStudy {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                HeatmapStudy::VolumeProfile(kind) => write!(f, "Volume Profile ({})", kind),
            }
        }
    }

    impl std::fmt::Display for ProfileKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ProfileKind::VisibleRange => write!(f, "Visible Range"),
                ProfileKind::FixedWindow { candles } => write!(f, "Fixed Window ({})", candles),
                ProfileKind::Fixed(n) => write!(f, "Fixed ({})", n),
            }
        }
    }

    pub const CLEANUP_THRESHOLD: usize = 1000;

    // Re-export HeatmapConfig for UI
    pub use crate::state::pane::HeatmapConfig as Config;
}

/// Kline-specific types
pub mod kline {
    pub use super::{
        CandlePosition, ClusterScaling, FootprintMode, FootprintStudyConfig, FootprintType,
        KlineDataPoint, KlineTrades, NPoc, PointOfControl,
    };

    // Re-export KlineConfig for UI
    pub use crate::state::pane::KlineConfig as Config;
}

/// UI indicator wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiIndicator {
    Kline(KlineIndicator),
    Heatmap(HeatmapIndicator),
}

impl From<KlineIndicator> for UiIndicator {
    fn from(indicator: KlineIndicator) -> Self {
        UiIndicator::Kline(indicator)
    }
}

impl From<HeatmapIndicator> for UiIndicator {
    fn from(indicator: HeatmapIndicator) -> Self {
        UiIndicator::Heatmap(indicator)
    }
}

impl std::fmt::Display for UiIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiIndicator::Kline(k) => write!(f, "{k}"),
            UiIndicator::Heatmap(h) => write!(f, "{h}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{Price, Quantity, Side, Timestamp};

    #[test]
    fn test_chart_basis_display() {
        let time_basis = ChartBasis::Time(Timeframe::M5);
        assert_eq!(format!("{}", time_basis), "M5");

        let tick_basis = ChartBasis::Tick(50);
        assert_eq!(format!("{}", tick_basis), "50T");
    }

    #[test]
    fn test_chart_data_creation() {
        let trades = vec![
            Trade::new(
                Timestamp(1000),
                Price::from_f32(100.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(2000),
                Price::from_f32(101.0),
                Quantity(5.0),
                Side::Sell,
            ),
        ];

        let candles = vec![];
        let chart_data = ChartData::from_trades(trades.clone(), candles);

        assert!(chart_data.has_trades());
        assert!(!chart_data.has_candles());
        assert!(!chart_data.has_depth());
        assert_eq!(chart_data.trade_count(), 2);
    }

    #[test]
    fn test_loading_status() {
        let status = LoadingStatus::Downloading {
            schema: DataSchema::Trades,
            days_total: 10,
            days_complete: 5,
            current_day: "2025-01-15".to_string(),
        };

        assert!(status.is_loading());
        assert!(!status.is_ready());
        assert!(!status.is_error());
    }
}
