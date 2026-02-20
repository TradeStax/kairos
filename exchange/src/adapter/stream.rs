//! Stream types for market data subscriptions.
//!
//! Defines the runtime [`StreamKind`] (resolved with ticker info) and its
//! persistence counterpart [`PersistStreamKind`] (serializable without
//! runtime-only fields). [`UniqueStreams`] collects and deduplicates streams,
//! and [`StreamSpecs`] summarizes active depth and kline subscriptions.

use crate::{FuturesTicker, FuturesTickerInfo, FuturesVenue, Timeframe};

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

/// Push frequency for orderbook updates
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize,
)]
pub enum PushFrequency {
    #[default]
    ServerDefault,
    Custom(Timeframe),
}

impl std::fmt::Display for PushFrequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushFrequency::ServerDefault => write!(f, "Server Default"),
            PushFrequency::Custom(tf) => write!(f, "{}", tf),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum StreamKind {
    Kline {
        ticker_info: FuturesTickerInfo,
        timeframe: Timeframe,
    },
    DepthAndTrades {
        ticker_info: FuturesTickerInfo,
        #[serde(default = "default_depth_aggr")]
        depth_aggr: StreamTicksize,
        push_freq: PushFrequency,
    },
}

impl StreamKind {
    pub fn ticker_info(&self) -> FuturesTickerInfo {
        match self {
            StreamKind::Kline { ticker_info, .. }
            | StreamKind::DepthAndTrades { ticker_info, .. } => *ticker_info,
        }
    }

    pub fn as_depth_stream(&self) -> Option<(FuturesTickerInfo, StreamTicksize, PushFrequency)> {
        match self {
            StreamKind::DepthAndTrades {
                ticker_info,
                depth_aggr,
                push_freq,
            } => Some((*ticker_info, *depth_aggr, *push_freq)),
            _ => None,
        }
    }

    pub fn as_kline_stream(&self) -> Option<(FuturesTickerInfo, Timeframe)> {
        match self {
            StreamKind::Kline {
                ticker_info,
                timeframe,
            } => Some((*ticker_info, *timeframe)),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct UniqueStreams {
    streams: FxHashMap<FuturesTickerInfo, FxHashSet<StreamKind>>,
    specs: Option<StreamSpecs>,
}

impl UniqueStreams {
    pub fn from<'a>(streams: impl Iterator<Item = &'a StreamKind>) -> Self {
        let mut unique_streams = UniqueStreams::default();
        for stream in streams {
            unique_streams.add(*stream);
        }
        unique_streams
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
        let depth_streams = self.depth_streams();
        let kline_streams = self.kline_streams();

        self.specs = Some(StreamSpecs {
            depth: depth_streams,
            kline: kline_streams,
        });
    }

    pub fn depth_streams(&self) -> Vec<(FuturesTickerInfo, StreamTicksize, PushFrequency)> {
        self.streams
            .values()
            .flatten()
            .filter_map(|stream| stream.as_depth_stream())
            .collect()
    }

    pub fn kline_streams(&self) -> Vec<(FuturesTickerInfo, Timeframe)> {
        self.streams
            .values()
            .flatten()
            .filter_map(|stream| stream.as_kline_stream())
            .collect()
    }

    pub fn combined(&self) -> Option<&StreamSpecs> {
        self.specs.as_ref()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum PersistStreamKind {
    Kline(PersistKline),
    DepthAndTrades(PersistDepth),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PersistDepth {
    pub ticker: FuturesTicker,
    #[serde(default = "default_depth_aggr")]
    pub depth_aggr: StreamTicksize,
    #[serde(default = "default_push_freq")]
    pub push_freq: PushFrequency,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PersistKline {
    pub ticker: FuturesTicker,
    pub timeframe: Timeframe,
}

impl From<StreamKind> for PersistStreamKind {
    fn from(s: StreamKind) -> Self {
        match s {
            StreamKind::Kline {
                ticker_info,
                timeframe,
            } => PersistStreamKind::Kline(PersistKline {
                ticker: ticker_info.ticker,
                timeframe,
            }),
            StreamKind::DepthAndTrades {
                ticker_info,
                depth_aggr,
                push_freq,
            } => PersistStreamKind::DepthAndTrades(PersistDepth {
                ticker: ticker_info.ticker,
                depth_aggr,
                push_freq,
            }),
        }
    }
}

impl PersistStreamKind {
    /// Try to convert into runtime StreamKind. `resolver` should return
    /// Some(FuturesTickerInfo) for a ticker, otherwise the conversion fails.
    pub fn into_stream_kind<F>(self, mut resolver: F) -> Result<StreamKind, String>
    where
        F: FnMut(&FuturesTicker) -> Option<FuturesTickerInfo>,
    {
        match self {
            PersistStreamKind::Kline(k) => resolver(&k.ticker)
                .map(|ti| StreamKind::Kline {
                    ticker_info: ti,
                    timeframe: k.timeframe,
                })
                .ok_or_else(|| format!("FuturesTickerInfo not found for {}", k.ticker)),
            PersistStreamKind::DepthAndTrades(d) => resolver(&d.ticker)
                .map(|ti| StreamKind::DepthAndTrades {
                    ticker_info: ti,
                    depth_aggr: d.depth_aggr,
                    push_freq: d.push_freq,
                })
                .ok_or_else(|| format!("FuturesTickerInfo not found for {}", d.ticker)),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum StreamTicksize {
    #[default]
    Client,
}

fn default_depth_aggr() -> StreamTicksize {
    StreamTicksize::Client
}

fn default_push_freq() -> PushFrequency {
    PushFrequency::ServerDefault
}

#[derive(Debug, Clone, Default)]
pub struct StreamSpecs {
    pub depth: Vec<(FuturesTickerInfo, StreamTicksize, PushFrequency)>,
    pub kline: Vec<(FuturesTickerInfo, Timeframe)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedStream {
    /// Streams that are persisted but needs to be resolved for use
    Waiting(Vec<PersistStreamKind>),
    /// Streams that are active and ready to use, but can't persist
    Ready(Vec<StreamKind>),
}

impl ResolvedStream {
    pub fn matches_stream(&self, stream: &StreamKind) -> bool {
        match self {
            ResolvedStream::Ready(existing) => existing.iter().any(|s| s == stream),
            _ => false,
        }
    }

    pub fn ready_iter_mut(&mut self) -> Option<impl Iterator<Item = &mut StreamKind>> {
        match self {
            ResolvedStream::Ready(streams) => Some(streams.iter_mut()),
            _ => None,
        }
    }

    pub fn ready_iter(&self) -> Option<impl Iterator<Item = &StreamKind>> {
        match self {
            ResolvedStream::Ready(streams) => Some(streams.iter()),
            _ => None,
        }
    }

    pub fn find_ready_map<F, T>(&self, f: F) -> Option<T>
    where
        F: FnMut(&StreamKind) -> Option<T>,
    {
        match self {
            ResolvedStream::Ready(streams) => streams.iter().find_map(f),
            _ => None,
        }
    }

    pub fn into_waiting(self) -> Vec<PersistStreamKind> {
        match self {
            ResolvedStream::Waiting(streams) => streams,
            ResolvedStream::Ready(streams) => streams
                .into_iter()
                .map(|s| match s {
                    StreamKind::DepthAndTrades {
                        ticker_info,
                        depth_aggr,
                        push_freq,
                    } => {
                        let persist_depth = PersistDepth {
                            ticker: ticker_info.ticker,
                            depth_aggr,
                            push_freq,
                        };
                        PersistStreamKind::DepthAndTrades(persist_depth)
                    }
                    StreamKind::Kline {
                        ticker_info,
                        timeframe,
                    } => {
                        let persist_kline = PersistKline {
                            ticker: ticker_info.ticker,
                            timeframe,
                        };
                        PersistStreamKind::Kline(persist_kline)
                    }
                })
                .collect(),
        }
    }

    pub fn waiting_to_resolve(&self) -> Option<&[PersistStreamKind]> {
        match self {
            ResolvedStream::Waiting(streams) => Some(streams),
            _ => None,
        }
    }

    pub fn ready_tickers(&self) -> Option<Vec<FuturesTickerInfo>> {
        match self {
            ResolvedStream::Ready(streams) => {
                Some(streams.iter().map(|s| s.ticker_info()).collect())
            }
            ResolvedStream::Waiting(_) => None,
        }
    }
}

impl IntoIterator for &ResolvedStream {
    type Item = StreamKind;
    type IntoIter = std::vec::IntoIter<StreamKind>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            ResolvedStream::Ready(streams) => streams.clone().into_iter(),
            ResolvedStream::Waiting(_) => vec![].into_iter(),
        }
    }
}

#[derive(Debug, Clone, Hash)]
pub struct StreamConfig<I> {
    pub id: I,
    pub venue: FuturesVenue,
    pub push_freq: PushFrequency,
}

impl<I> StreamConfig<I> {
    pub fn new(id: I, venue: FuturesVenue, push_freq: PushFrequency) -> Self {
        Self {
            id,
            venue,
            push_freq,
        }
    }
}
