//! Stream deduplication across panes.
//!
//! [`UniqueStreams`] collects streams from all active panes and deduplicates
//! them by ticker, producing a single [`StreamSpecs`] for subscription management.

use super::kind::{PushFrequency, StreamKind, StreamTicksize};
use crate::domain::futures::{FuturesTickerInfo, Timeframe};
use rustc_hash::{FxHashMap, FxHashSet};

/// Deduplicates streams across panes, grouped by ticker.
///
/// Maintains a cached [`StreamSpecs`] that is updated on every `add` call.
#[derive(Debug, Default)]
pub struct UniqueStreams {
    streams: FxHashMap<FuturesTickerInfo, FxHashSet<StreamKind>>,
    specs: Option<StreamSpecs>,
}

impl UniqueStreams {
    /// Creates a `UniqueStreams` from an iterator of stream references
    pub fn from<'a>(streams: impl Iterator<Item = &'a StreamKind>) -> Self {
        let mut unique = UniqueStreams::default();
        for stream in streams {
            unique.add(*stream);
        }
        unique
    }

    /// Adds a stream, deduplicating by ticker and stream kind
    pub fn add(&mut self, stream: StreamKind) {
        let ticker_info = stream.ticker_info();
        self.streams.entry(ticker_info).or_default().insert(stream);
        self.update_specs();
    }

    /// Adds multiple streams from an iterator
    pub fn extend<'a>(&mut self, streams: impl IntoIterator<Item = &'a StreamKind>) {
        for stream in streams {
            self.add(*stream);
        }
    }

    /// Rebuilds the cached specs from current streams
    fn update_specs(&mut self) {
        #[cfg(feature = "heatmap")]
        let depth = self.depth_streams();
        #[cfg(not(feature = "heatmap"))]
        let depth = vec![];
        let kline = self.kline_streams();
        self.specs = Some(StreamSpecs { depth, kline });
    }

    /// Returns all unique depth stream specifications
    #[cfg(feature = "heatmap")]
    #[must_use]
    pub fn depth_streams(&self) -> Vec<(FuturesTickerInfo, StreamTicksize, PushFrequency)> {
        self.streams
            .values()
            .flatten()
            .filter_map(|s| s.as_depth_stream())
            .collect()
    }

    /// Returns all unique kline stream specifications
    #[must_use]
    pub fn kline_streams(&self) -> Vec<(FuturesTickerInfo, Timeframe)> {
        self.streams
            .values()
            .flatten()
            .filter_map(|s| s.as_kline_stream())
            .collect()
    }

    /// Returns the cached combined stream specs, if any streams have been added
    #[must_use]
    pub fn combined(&self) -> Option<&StreamSpecs> {
        self.specs.as_ref()
    }
}

/// Combined depth and kline stream specifications for subscription.
#[derive(Debug, Clone, Default)]
pub struct StreamSpecs {
    /// Unique depth stream subscriptions
    pub depth: Vec<(FuturesTickerInfo, StreamTicksize, PushFrequency)>,
    /// Unique kline stream subscriptions
    pub kline: Vec<(FuturesTickerInfo, Timeframe)>,
}

/// Generic stream configuration holder, parameterized by ID type.
#[derive(Debug, Clone, Hash)]
pub struct StreamConfig<I> {
    /// Stream identifier
    pub id: I,
    /// Target venue
    pub venue: crate::domain::futures::FuturesVenue,
    /// Push frequency for updates
    pub push_freq: PushFrequency,
}

impl<I> StreamConfig<I> {
    /// Creates a new stream configuration
    pub fn new(
        id: I,
        venue: crate::domain::futures::FuturesVenue,
        push_freq: PushFrequency,
    ) -> Self {
        Self {
            id,
            venue,
            push_freq,
        }
    }
}
