//! Resolved stream — Waiting (persisted) vs Ready (active)

#[cfg(feature = "heatmap")]
use super::kind::PersistDepth;
use super::kind::{PersistKline, PersistStreamKind, StreamKind};
use crate::domain::futures::FuturesTickerInfo;

#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedStream {
    /// Persisted stream waiting to be resolved with ticker info
    Waiting(Vec<PersistStreamKind>),
    /// Active stream ready to use
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
                    StreamKind::Kline {
                        ticker_info,
                        timeframe,
                    } => PersistStreamKind::Kline(PersistKline {
                        ticker: ticker_info.ticker,
                        timeframe,
                    }),
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
