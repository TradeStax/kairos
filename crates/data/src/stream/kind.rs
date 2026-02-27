//! Stream kind types — runtime and serializable stream variants.
//!
//! [`StreamKind`] holds a fully-resolved `FuturesTickerInfo` for runtime use.
//! [`PersistStreamKind`] holds only a `FuturesTicker` symbol for serialization.
//! Conversion between the two requires a ticker-info resolver function.

use crate::domain::futures::{FuturesTicker, FuturesTickerInfo, Timeframe};
use serde::{Deserialize, Serialize};

/// Push frequency for order book depth updates.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum PushFrequency {
    /// Use the server's default update interval
    #[default]
    ServerDefault,
    /// Push at a custom timeframe interval
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

/// Depth tick-size aggregation mode.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum StreamTicksize {
    /// Use client-side tick-size aggregation
    #[default]
    Client,
}

#[cfg(feature = "heatmap")]
fn default_depth_aggr() -> StreamTicksize {
    StreamTicksize::Client
}

#[cfg(feature = "heatmap")]
fn default_push_freq() -> PushFrequency {
    PushFrequency::ServerDefault
}

/// Runtime-resolved stream with full ticker info.
///
/// Variants hold complete [`FuturesTickerInfo`] needed for subscription
/// and data processing. Created by resolving a [`PersistStreamKind`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum StreamKind {
    Kline {
        ticker_info: FuturesTickerInfo,
        timeframe: Timeframe,
    },
    #[cfg(feature = "heatmap")]
    DepthAndTrades {
        ticker_info: FuturesTickerInfo,
        #[serde(default = "default_depth_aggr")]
        depth_aggr: StreamTicksize,
        push_freq: PushFrequency,
    },
}

impl StreamKind {
    /// Returns the ticker info for this stream
    #[must_use]
    pub fn ticker_info(&self) -> FuturesTickerInfo {
        match self {
            StreamKind::Kline { ticker_info, .. } => *ticker_info,
            #[cfg(feature = "heatmap")]
            StreamKind::DepthAndTrades { ticker_info, .. } => *ticker_info,
        }
    }

    /// Extracts depth stream parameters, if this is a depth stream
    #[cfg(feature = "heatmap")]
    #[must_use]
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

    /// Extracts kline stream parameters, if this is a kline stream
    #[must_use]
    pub fn as_kline_stream(&self) -> Option<(FuturesTickerInfo, Timeframe)> {
        match self {
            StreamKind::Kline {
                ticker_info,
                timeframe,
            } => Some((*ticker_info, *timeframe)),
            #[cfg(feature = "heatmap")]
            _ => None,
        }
    }
}

/// Serializable stream kind that stores only the ticker symbol.
///
/// Used for layout persistence. Must be resolved into a [`StreamKind`] via
/// [`into_stream_kind`](Self::into_stream_kind) before use at runtime.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum PersistStreamKind {
    /// Serialized kline stream
    Kline(PersistKline),
    /// Serialized depth+trades stream
    #[cfg(feature = "heatmap")]
    DepthAndTrades(PersistDepth),
}

/// Serializable depth stream configuration.
#[cfg(feature = "heatmap")]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PersistDepth {
    /// Ticker symbol
    pub ticker: FuturesTicker,
    /// Depth aggregation mode
    #[serde(default = "default_depth_aggr")]
    pub depth_aggr: StreamTicksize,
    /// Push frequency for depth updates
    #[serde(default = "default_push_freq")]
    pub push_freq: PushFrequency,
}

/// Serializable kline stream configuration.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PersistKline {
    /// Ticker symbol
    pub ticker: FuturesTicker,
    /// Candle timeframe
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
            #[cfg(feature = "heatmap")]
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
    /// Converts into a runtime [`StreamKind`] by resolving the ticker symbol.
    ///
    /// The resolver function maps a `FuturesTicker` to its full `FuturesTickerInfo`.
    /// Returns an error if the ticker cannot be resolved.
    pub fn into_stream_kind<F>(self, mut resolver: F) -> Result<StreamKind, crate::Error>
    where
        F: FnMut(&FuturesTicker) -> Option<FuturesTickerInfo>,
    {
        match self {
            PersistStreamKind::Kline(k) => resolver(&k.ticker)
                .map(|ti| StreamKind::Kline {
                    ticker_info: ti,
                    timeframe: k.timeframe,
                })
                .ok_or_else(|| {
                    crate::Error::Config(format!("FuturesTickerInfo not found for {}", k.ticker))
                }),
            #[cfg(feature = "heatmap")]
            PersistStreamKind::DepthAndTrades(d) => resolver(&d.ticker)
                .map(|ti| StreamKind::DepthAndTrades {
                    ticker_info: ti,
                    depth_aggr: d.depth_aggr,
                    push_freq: d.push_freq,
                })
                .ok_or_else(|| {
                    crate::Error::Config(format!("FuturesTickerInfo not found for {}", d.ticker))
                }),
        }
    }
}
