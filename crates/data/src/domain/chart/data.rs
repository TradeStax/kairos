//! Chart data containers — in-memory trade and candle storage with gap tracking
//! and multi-feed merge results.

use crate::domain::core::types::{FeedId, TimeRange, Timestamp};
#[cfg(feature = "heatmap")]
use crate::domain::market::entities::Depth;
use crate::domain::market::entities::{Candle, Trade};

// ── DataGap ─────────────────────────────────────────────────────────────

/// Kind of data gap detected during multi-feed merging.
#[derive(Debug, Clone, PartialEq)]
pub enum DataGapKind {
    /// No data available from any feed
    NoData,
    /// Gap due to market being closed
    MarketClosed,
    /// Only some feeds contributed data
    PartialCoverage {
        /// Feeds that did provide data during this period
        available_feeds: Vec<FeedId>,
    },
}

/// A gap in the data timeline between two timestamps.
#[derive(Debug, Clone, PartialEq)]
pub struct DataGap {
    /// Gap start (inclusive)
    pub start: Timestamp,
    /// Gap end (inclusive)
    pub end: Timestamp,
    /// Classification of the gap
    pub kind: DataGapKind,
}

impl DataGap {
    /// Create a new data gap
    #[must_use]
    pub fn new(start: Timestamp, end: Timestamp, kind: DataGapKind) -> Self {
        Self { start, end, kind }
    }

    /// Return the gap duration in milliseconds
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        self.end.0.saturating_sub(self.start.0)
    }
}

// ── DataSegment ─────────────────────────────────────────────────────────

/// A contiguous segment of trade data from a single feed.
#[derive(Debug, Clone)]
pub struct DataSegment {
    /// Source feed identifier
    pub feed_id: FeedId,
    /// Segment start timestamp
    pub start: Timestamp,
    /// Segment end timestamp
    pub end: Timestamp,
    /// Trades in this segment
    pub trades: Vec<Trade>,
}

/// Result of merging data from multiple feeds.
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// Merged, time-sorted trades from all feeds
    pub trades: Vec<Trade>,
    /// Gaps detected during merging
    pub gaps: Vec<DataGap>,
    /// Feed IDs that contributed data
    pub feed_ids: Vec<FeedId>,
}

// ── ChartData ───────────────────────────────────────────────────────────

/// In-memory representation of all data backing a single chart.
///
/// Trades are the primary source of truth; candles are derived from them.
#[derive(Debug, Clone)]
pub struct ChartData {
    /// Raw tick-by-tick trades (primary source of truth)
    pub trades: Vec<Trade>,
    /// Aggregated candles (derived from trades)
    pub candles: Vec<Candle>,
    /// Depth snapshots (only present for heatmap charts)
    #[cfg(feature = "heatmap")]
    pub depth_snapshots: Option<Vec<Depth>>,
    /// Data gaps detected during multi-feed merging
    pub gaps: Vec<DataGap>,
    /// Time range of the data
    pub time_range: TimeRange,
}

impl ChartData {
    /// Create chart data from trades and pre-aggregated candles
    #[must_use]
    pub fn from_trades(trades: Vec<Trade>, candles: Vec<Candle>) -> Self {
        let time_range = if let (Some(first), Some(last)) = (trades.first(), trades.last()) {
            TimeRange::new(first.time, last.time)
                .expect("invariant: first trade time <= last trade time in sorted vec")
        } else {
            TimeRange::new(Timestamp(0), Timestamp(0))
                .expect("invariant: equal timestamps are a valid empty range")
        };

        Self {
            trades,
            candles,
            #[cfg(feature = "heatmap")]
            depth_snapshots: None,
            gaps: Vec::new(),
            time_range,
        }
    }

    /// Attach depth snapshots (builder pattern)
    #[cfg(feature = "heatmap")]
    #[must_use]
    pub fn with_depth(mut self, depth_snapshots: Vec<Depth>) -> Self {
        self.depth_snapshots = Some(depth_snapshots);
        self
    }

    /// Attach data gaps (builder pattern)
    #[must_use]
    pub fn with_gaps(mut self, gaps: Vec<DataGap>) -> Self {
        self.gaps = gaps;
        self
    }

    /// Return `true` if any data gaps were detected
    #[must_use]
    pub fn has_gaps(&self) -> bool {
        !self.gaps.is_empty()
    }

    /// Return `true` if trades are present
    #[must_use]
    pub fn has_trades(&self) -> bool {
        !self.trades.is_empty()
    }

    /// Return `true` if candles are present
    #[must_use]
    pub fn has_candles(&self) -> bool {
        !self.candles.is_empty()
    }

    /// Return `true` if depth snapshots are present and non-empty
    #[cfg(feature = "heatmap")]
    #[must_use]
    pub fn has_depth(&self) -> bool {
        self.depth_snapshots.as_ref().is_some_and(|d| !d.is_empty())
    }

    /// Return `false` (depth is never available without the `heatmap` feature)
    #[cfg(not(feature = "heatmap"))]
    #[must_use]
    pub fn has_depth(&self) -> bool {
        false
    }

    /// Return the number of trades
    #[must_use]
    pub fn trade_count(&self) -> usize {
        self.trades.len()
    }

    /// Return the number of candles
    #[must_use]
    pub fn candle_count(&self) -> usize {
        self.candles.len()
    }

    /// Estimate total memory usage in bytes
    #[must_use]
    pub fn memory_usage(&self) -> usize {
        let trade_size = std::mem::size_of::<Trade>();
        let candle_size = std::mem::size_of::<Candle>();

        let trades_mem = self.trades.len() * trade_size;
        let candles_mem = self.candles.len() * candle_size;
        #[cfg(feature = "heatmap")]
        let depth_mem = self.depth_snapshots.as_ref().map_or(0, |d| d.len() * 1024);
        #[cfg(not(feature = "heatmap"))]
        let depth_mem = 0usize;
        let gaps_mem = self.gaps.len() * std::mem::size_of::<DataGap>();

        trades_mem + candles_mem + depth_mem + gaps_mem
    }
}

// ── LoadingStatus ───────────────────────────────────────────────────────

/// Progress state of chart data loading.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadingStatus {
    /// No operation in progress
    Idle,
    /// Downloading data from a remote source
    Downloading {
        /// Schema being downloaded
        schema: DataSchema,
        /// Total number of days to download
        days_total: usize,
        /// Number of days completed
        days_complete: usize,
        /// Label for the day currently being downloaded
        current_day: String,
    },
    /// Loading data from local cache
    LoadingFromCache {
        /// Schema being loaded
        schema: DataSchema,
        /// Total number of days to load
        days_total: usize,
        /// Number of days loaded so far
        days_loaded: usize,
        /// Total items loaded across all days
        items_loaded: usize,
        /// Overall progress fraction `[0.0, 1.0]` including sub-day
        /// granularity. When `Some`, this takes priority over the
        /// integer `days_loaded / days_total` ratio for progress bars.
        progress_fraction: Option<f32>,
    },
    /// Building derived data structures
    Building {
        /// Description of the current operation
        operation: String,
        /// Progress fraction `[0.0, 1.0]`
        progress: f32,
    },
    /// Data is ready for display
    Ready,
    /// Loading failed
    Error {
        /// Human-readable error message
        message: String,
    },
}

impl LoadingStatus {
    /// Return `true` if any loading operation is in progress
    #[must_use]
    pub fn is_loading(&self) -> bool {
        matches!(
            self,
            LoadingStatus::Downloading { .. }
                | LoadingStatus::LoadingFromCache { .. }
                | LoadingStatus::Building { .. }
        )
    }

    /// Return `true` if data is ready
    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self, LoadingStatus::Ready)
    }

    /// Return `true` if loading ended with an error
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, LoadingStatus::Error { .. })
    }
}

// ── DataSchema ──────────────────────────────────────────────────────────

/// Data schema identifier for download and cache operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSchema {
    /// Tick-by-tick trade data
    Trades,
    /// Market-by-price 10-level depth
    MBP10,
    /// OHLCV bar data
    OHLCV,
    /// Options chain data
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
    use crate::domain::core::types::{Price, Quantity, Side};

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
