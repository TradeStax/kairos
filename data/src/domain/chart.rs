//! Chart Domain Model
//!
//! Domain types for chart configuration and data.
//! These are pure domain concepts without any UI/rendering concerns.

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

/// Chart type (candlestick, footprint, heatmap)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartType {
    /// Standard candlestick chart
    Candlestick,
    /// Line chart
    Line,
    /// Heikin-Ashi candlestick chart
    HeikinAshi,
    /// Footprint chart (trade clusters)
    Footprint,
    /// Heatmap (orderbook visualization)
    Heatmap,
}

impl std::fmt::Display for ChartType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChartType::Candlestick => write!(f, "Candlestick"),
            ChartType::Line => write!(f, "Line"),
            ChartType::HeikinAshi => write!(f, "Heikin-Ashi"),
            ChartType::Footprint => write!(f, "Footprint"),
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
