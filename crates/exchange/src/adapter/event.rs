//! Events emitted by exchange adapters.
//!
//! [`Event`] covers both historical replay events (depth snapshots, klines)
//! and live streaming events (connection status, real-time data).

use crate::FuturesVenue;
use crate::types::{Depth, Kline, Trade};
use std::sync::Arc;

use super::stream::StreamKind;

/// Events for both historical data replay and live streaming
#[derive(Debug, Clone)]
pub enum Event {
    /// Historical depth snapshot and trades for a specific time
    HistoricalDepth(u64, Arc<Depth>, Box<[Trade]>),
    /// Historical kline/candle data
    HistoricalKline(Kline),

    /// WebSocket connection established
    Connected(FuturesVenue),
    /// WebSocket connection closed
    Disconnected(FuturesVenue, String),
    /// WebSocket connection lost (will attempt reconnection)
    ConnectionLost,
    /// Live depth snapshot with trades
    DepthReceived(StreamKind, u64, Arc<Depth>, Box<[Trade]>),
    /// Live kline update
    KlineReceived(StreamKind, Kline),
    /// Individual trade update (for real-time feed)
    TradeReceived(StreamKind, Trade),
}
