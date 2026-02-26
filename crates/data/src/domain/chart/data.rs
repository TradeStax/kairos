//! Chart Data Types

use crate::domain::core::types::{FeedId, TimeRange, Timestamp};
#[cfg(feature = "heatmap")]
use crate::domain::market::entities::Depth;
use crate::domain::market::entities::{Candle, Trade};

/// Kind of data gap detected during multi-feed merging
#[derive(Debug, Clone, PartialEq)]
pub enum DataGapKind {
    NoData,
    MarketClosed,
    PartialCoverage { available_feeds: Vec<FeedId> },
}

/// A gap in the data timeline
#[derive(Debug, Clone, PartialEq)]
pub struct DataGap {
    pub start: Timestamp,
    pub end: Timestamp,
    pub kind: DataGapKind,
}

impl DataGap {
    pub fn new(start: Timestamp, end: Timestamp, kind: DataGapKind) -> Self {
        Self { start, end, kind }
    }

    pub fn duration_ms(&self) -> u64 {
        self.end.0.saturating_sub(self.start.0)
    }
}

/// A segment of data from a specific feed
#[derive(Debug, Clone)]
pub struct DataSegment {
    pub feed_id: FeedId,
    pub start: Timestamp,
    pub end: Timestamp,
    pub trades: Vec<Trade>,
}

/// Result of merging data from multiple feeds
#[derive(Debug, Clone)]
pub struct MergeResult {
    pub trades: Vec<Trade>,
    pub gaps: Vec<DataGap>,
    pub feed_ids: Vec<FeedId>,
}

/// Chart data — in-memory representation of all chart data
#[derive(Debug, Clone)]
pub struct ChartData {
    /// Raw tick-by-tick trades (PRIMARY SOURCE OF TRUTH)
    pub trades: Vec<Trade>,
    /// Aggregated candles (DERIVED from trades)
    pub candles: Vec<Candle>,
    /// Depth snapshots (OPTIONAL, only for heatmap)
    #[cfg(feature = "heatmap")]
    pub depth_snapshots: Option<Vec<Depth>>,
    /// Data gaps detected during multi-feed merging
    pub gaps: Vec<DataGap>,
    /// Time range of the data
    pub time_range: TimeRange,
}

impl ChartData {
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

    #[cfg(feature = "heatmap")]
    pub fn with_depth(mut self, depth_snapshots: Vec<Depth>) -> Self {
        self.depth_snapshots = Some(depth_snapshots);
        self
    }

    pub fn with_gaps(mut self, gaps: Vec<DataGap>) -> Self {
        self.gaps = gaps;
        self
    }

    pub fn has_gaps(&self) -> bool {
        !self.gaps.is_empty()
    }

    pub fn has_trades(&self) -> bool {
        !self.trades.is_empty()
    }

    pub fn has_candles(&self) -> bool {
        !self.candles.is_empty()
    }

    #[cfg(feature = "heatmap")]
    pub fn has_depth(&self) -> bool {
        self.depth_snapshots.as_ref().is_some_and(|d| !d.is_empty())
    }

    #[cfg(not(feature = "heatmap"))]
    pub fn has_depth(&self) -> bool {
        false
    }

    pub fn trade_count(&self) -> usize {
        self.trades.len()
    }

    pub fn candle_count(&self) -> usize {
        self.candles.len()
    }

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

/// Loading status for chart data
#[derive(Debug, Clone, PartialEq)]
pub enum LoadingStatus {
    Idle,
    Downloading {
        schema: DataSchema,
        days_total: usize,
        days_complete: usize,
        current_day: String,
    },
    LoadingFromCache {
        schema: DataSchema,
        days_total: usize,
        days_loaded: usize,
        items_loaded: usize,
    },
    Building {
        operation: String,
        progress: f32,
    },
    Ready,
    Error {
        message: String,
    },
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
