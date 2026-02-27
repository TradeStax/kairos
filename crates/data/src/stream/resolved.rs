//! Resolved stream — two-state wrapper for stream lifecycle.
//!
//! A [`ResolvedStream`] is either `Waiting` (persisted, needs ticker info
//! resolution) or `Ready` (fully resolved, active for subscription).

#[cfg(feature = "heatmap")]
use super::kind::PersistDepth;
use super::kind::{PersistKline, PersistStreamKind, StreamKind};
use crate::domain::futures::FuturesTickerInfo;

/// A stream in one of two lifecycle states: waiting for resolution or ready.
///
/// Panes start with `Waiting` after deserialization and transition to `Ready`
/// once ticker info is available. Reverts to `Waiting` for serialization via
/// [`into_waiting`](Self::into_waiting).
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedStream {
    /// Persisted stream waiting to be resolved with ticker info
    Waiting(Vec<PersistStreamKind>),
    /// Fully resolved stream ready for subscription
    Ready(Vec<StreamKind>),
}

impl ResolvedStream {
    /// Returns `true` if any ready stream matches the given stream kind
    #[must_use]
    pub fn matches_stream(&self, stream: &StreamKind) -> bool {
        match self {
            ResolvedStream::Ready(existing) => existing.iter().any(|s| s == stream),
            _ => false,
        }
    }

    /// Returns a mutable iterator over ready streams, or `None` if waiting
    pub fn ready_iter_mut(&mut self) -> Option<impl Iterator<Item = &mut StreamKind>> {
        match self {
            ResolvedStream::Ready(streams) => Some(streams.iter_mut()),
            _ => None,
        }
    }

    /// Returns an iterator over ready streams, or `None` if waiting
    #[must_use]
    pub fn ready_iter(&self) -> Option<impl Iterator<Item = &StreamKind>> {
        match self {
            ResolvedStream::Ready(streams) => Some(streams.iter()),
            _ => None,
        }
    }

    /// Finds the first ready stream matching the predicate and maps it
    pub fn find_ready_map<F, T>(&self, f: F) -> Option<T>
    where
        F: FnMut(&StreamKind) -> Option<T>,
    {
        match self {
            ResolvedStream::Ready(streams) => streams.iter().find_map(f),
            _ => None,
        }
    }

    /// Converts this stream back to serializable form.
    ///
    /// If already `Waiting`, returns the inner vec. If `Ready`, strips
    /// ticker info down to just the ticker symbol.
    #[must_use]
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

    /// Returns the waiting streams slice, or `None` if already resolved
    #[must_use]
    pub fn waiting_to_resolve(&self) -> Option<&[PersistStreamKind]> {
        match self {
            ResolvedStream::Waiting(streams) => Some(streams),
            _ => None,
        }
    }

    /// Returns ticker info for all ready streams, or `None` if waiting
    #[must_use]
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
