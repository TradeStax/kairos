//! UniqueStreams — deduplicates streams across panes

use super::kind::{PushFrequency, StreamKind, StreamTicksize};
use crate::domain::futures::{FuturesTickerInfo, Timeframe};
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, Default)]
pub struct UniqueStreams {
    streams: FxHashMap<FuturesTickerInfo, FxHashSet<StreamKind>>,
    specs: Option<StreamSpecs>,
}

impl UniqueStreams {
    pub fn from<'a>(streams: impl Iterator<Item = &'a StreamKind>) -> Self {
        let mut unique = UniqueStreams::default();
        for stream in streams {
            unique.add(*stream);
        }
        unique
    }

    pub fn add(&mut self, stream: StreamKind) {
        let ticker_info = stream.ticker_info();
        self.streams.entry(ticker_info).or_default().insert(stream);
        self.update_specs();
    }

    pub fn extend<'a>(&mut self, streams: impl IntoIterator<Item = &'a StreamKind>) {
        for stream in streams {
            self.add(*stream);
        }
    }

    fn update_specs(&mut self) {
        #[cfg(feature = "heatmap")]
        let depth = self.depth_streams();
        #[cfg(not(feature = "heatmap"))]
        let depth = vec![];
        let kline = self.kline_streams();
        self.specs = Some(StreamSpecs { depth, kline });
    }

    #[cfg(feature = "heatmap")]
    pub fn depth_streams(&self) -> Vec<(FuturesTickerInfo, StreamTicksize, PushFrequency)> {
        self.streams
            .values()
            .flatten()
            .filter_map(|s| s.as_depth_stream())
            .collect()
    }

    pub fn kline_streams(&self) -> Vec<(FuturesTickerInfo, Timeframe)> {
        self.streams
            .values()
            .flatten()
            .filter_map(|s| s.as_kline_stream())
            .collect()
    }

    pub fn combined(&self) -> Option<&StreamSpecs> {
        self.specs.as_ref()
    }
}

#[derive(Debug, Clone, Default)]
pub struct StreamSpecs {
    pub depth: Vec<(FuturesTickerInfo, StreamTicksize, PushFrequency)>,
    pub kline: Vec<(FuturesTickerInfo, Timeframe)>,
}

/// Generic stream configuration holder
#[derive(Debug, Clone, Hash)]
pub struct StreamConfig<I> {
    pub id: I,
    pub venue: crate::domain::futures::FuturesVenue,
    pub push_freq: PushFrequency,
}

impl<I> StreamConfig<I> {
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
