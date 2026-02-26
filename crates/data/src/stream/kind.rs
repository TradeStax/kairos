//! Stream kind types

use crate::domain::futures::{FuturesTicker, FuturesTickerInfo, Timeframe};
use serde::{Deserialize, Serialize};

/// Push frequency for orderbook updates
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
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

/// Depth aggregation mode
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum StreamTicksize {
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

/// Runtime-resolved stream (holds full FuturesTickerInfo)
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
    pub fn ticker_info(&self) -> FuturesTickerInfo {
        match self {
            StreamKind::Kline { ticker_info, .. } => *ticker_info,
            #[cfg(feature = "heatmap")]
            StreamKind::DepthAndTrades { ticker_info, .. } => *ticker_info,
        }
    }

    #[cfg(feature = "heatmap")]
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
            #[cfg(feature = "heatmap")]
            _ => None,
        }
    }
}

/// Serializable stream kind (holds only FuturesTicker, not full info)
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum PersistStreamKind {
    Kline(PersistKline),
    #[cfg(feature = "heatmap")]
    DepthAndTrades(PersistDepth),
}

#[cfg(feature = "heatmap")]
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
    /// Try to convert into runtime StreamKind.
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
