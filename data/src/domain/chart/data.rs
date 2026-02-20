//! Chart data types: segments, gaps, and merged data

use crate::domain::entities::{Candle, DepthSnapshot, Trade};
use crate::domain::types::{TimeRange, Timestamp};
use crate::feed::FeedId;

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
        let time_range = if let (Some(first), Some(last)) =
            (trades.first(), trades.last())
        {
            TimeRange::new(first.time, last.time)
        } else {
            TimeRange::new(Timestamp(0), Timestamp(0))
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
        let depth_mem = self
            .depth_snapshots
            .as_ref()
            .map_or(0, |d| d.len() * 1024); // ~1KB per snapshot

        let gaps_mem = self.gaps.len() * std::mem::size_of::<DataGap>();

        trades_mem + candles_mem + depth_mem + gaps_mem
    }
}
