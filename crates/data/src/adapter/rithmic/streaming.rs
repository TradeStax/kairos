//! Rithmic live-data streaming loop.
//!
//! [`RithmicStream`] wraps a ticker plant handle and runs an async
//! receive loop that transforms [`RithmicMessage`] variants into
//! [`DataEvent`]s for the application layer. Handles trade, depth,
//! forced-logout, and connection-error messages with periodic stats
//! logging.

use super::mapper;
use super::plants::RithmicTickerPlantHandle;
use super::protocol::RithmicMessage;
use crate::domain::{FuturesTicker, FuturesTickerInfo};
use crate::event::DataEvent;
#[cfg(feature = "heatmap")]
use crate::stream::kind::{PushFrequency, StreamKind, StreamTicksize};
use rustc_hash::FxHashMap;

/// Rithmic streaming subscription.
///
/// Wraps a [`RithmicTickerPlantHandle`] and produces [`DataEvent`]s
/// from the broadcast subscription receiver.
pub struct RithmicStream {
    handle: RithmicTickerPlantHandle,
}

impl RithmicStream {
    /// Creates a new stream from a ticker plant handle
    pub fn new(handle: RithmicTickerPlantHandle) -> Self {
        Self { handle }
    }

    /// Run the streaming loop, sending DataEvents to the provided channel.
    ///
    /// `ticker_map` maps Rithmic symbol strings (e.g. "ES", "NQ") to
    /// their `FuturesTickerInfo`.
    ///
    /// This consumes self and runs until the connection is lost
    /// or the sender is dropped.
    pub async fn run(
        mut self,
        ticker_map: FxHashMap<String, FuturesTickerInfo>,
        event_tx: tokio::sync::mpsc::UnboundedSender<DataEvent>,
    ) {
        log::info!(
            "Rithmic streaming loop started (watching {} ticker(s))",
            ticker_map.len()
        );

        let mut trade_count: u64 = 0;
        #[allow(unused_mut)]
        let mut depth_count: u64 = 0;
        let mut other_count: u64 = 0;
        let mut last_stats = std::time::Instant::now();
        let started = std::time::Instant::now();
        let mut first_trade_logged = false;

        loop {
            // Use a timeout so we can log periodic diagnostics even
            // when no messages arrive.
            let recv_result = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                self.handle.subscription_receiver.recv(),
            )
            .await;

            let response = match recv_result {
                Ok(Ok(response)) => response,
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(n))) => {
                    log::warn!("Rithmic stream: receiver lagged, missed {} messages", n);
                    // Recoverable — continue from current position
                    continue;
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    log::error!(
                        "Rithmic broadcast channel closed \
                         (ticker plant exited?)"
                    );
                    break;
                }
                Err(_timeout) => {
                    let elapsed = started.elapsed().as_secs();
                    log::warn!(
                        "Rithmic stream: no messages for 30s \
                         (total: {} trades, {} depth, {} other, \
                         uptime: {}s)",
                        trade_count,
                        depth_count,
                        other_count,
                        elapsed,
                    );
                    continue;
                }
            };

            if let Some(err) = &response.error {
                log::warn!("Rithmic stream error: {}", err);
                continue;
            }

            match &response.message {
                RithmicMessage::LastTrade(lt) => {
                    let Some(ticker) = resolve_ticker(lt.symbol.as_deref(), &ticker_map) else {
                        continue;
                    };
                    match mapper::map_last_trade(&response.message) {
                        Some(trade) => {
                            trade_count += 1;
                            if !first_trade_logged {
                                first_trade_logged = true;
                                log::info!(
                                    "Rithmic stream: first live trade — \
                                     {} @ {} ({:?})",
                                    ticker.as_str(),
                                    trade.price,
                                    trade.side,
                                );
                            }
                            if event_tx
                                .send(DataEvent::TradeReceived { ticker, trade })
                                .is_err()
                            {
                                log::info!(
                                    "Event channel closed, stopping \
                                     Rithmic stream"
                                );
                                break;
                            }
                        }
                        None => {
                            log::debug!(
                                "Rithmic stream: map_last_trade returned \
                                 None for {:?}",
                                lt.symbol
                            );
                        }
                    }
                }
                #[cfg(feature = "heatmap")]
                RithmicMessage::BestBidOffer(bbo) => {
                    let Some(ticker) = resolve_ticker(bbo.symbol.as_deref(), &ticker_map) else {
                        continue;
                    };
                    if let Some(depth) = mapper::map_bbo_to_depth(&response.message) {
                        depth_count += 1;
                        if event_tx
                            .send(DataEvent::DepthReceived { ticker, depth })
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                #[cfg(feature = "heatmap")]
                RithmicMessage::OrderBook(ob) => {
                    let Some(ticker) = resolve_ticker(ob.symbol.as_deref(), &ticker_map) else {
                        continue;
                    };
                    if let Some(depth) = mapper::map_orderbook_to_depth(&response.message) {
                        depth_count += 1;
                        if event_tx
                            .send(DataEvent::DepthReceived { ticker, depth })
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                RithmicMessage::ForcedLogout(_) => {
                    log::warn!("Rithmic forced logout");
                    break;
                }
                RithmicMessage::ConnectionError => {
                    log::error!("Rithmic connection error");
                    break;
                }
                RithmicMessage::HeartbeatTimeout => {
                    log::warn!("Rithmic heartbeat timeout");
                    break;
                }
                _other => {
                    other_count += 1;
                }
            }

            // Periodic stats every 60s
            if last_stats.elapsed().as_secs() >= 60 {
                log::info!(
                    "Rithmic stream stats: {} trades, {} depth, \
                     {} other (uptime: {}s)",
                    trade_count,
                    depth_count,
                    other_count,
                    started.elapsed().as_secs(),
                );
                last_stats = std::time::Instant::now();
            }
        }

        log::info!(
            "Rithmic streaming loop ended (total: {} trades, {} depth, \
             {} other, uptime: {}s)",
            trade_count,
            depth_count,
            other_count,
            started.elapsed().as_secs(),
        );
    }
}

/// Resolves a Rithmic symbol string to a [`FuturesTicker`].
///
/// Tries exact match first, then prefix match (e.g. "ESH6" matching
/// "ES"). Returns `None` with a warning if no match is found.
fn resolve_ticker(
    symbol: Option<&str>,
    ticker_map: &FxHashMap<String, FuturesTickerInfo>,
) -> Option<FuturesTicker> {
    let info = if let Some(sym) = symbol {
        if let Some(info) = ticker_map.get(sym) {
            *info
        } else {
            let found = ticker_map
                .iter()
                .find(|(key, _)| sym.starts_with(key.as_str()));
            if let Some((_, info)) = found {
                *info
            } else {
                log::warn!("Unknown Rithmic symbol '{}', no matching ticker info", sym);
                return None;
            }
        }
    } else {
        *ticker_map.values().next()?
    };

    Some(info.ticker)
}

/// Builds a [`StreamKind`] for a Rithmic depth-and-trades subscription
#[cfg(feature = "heatmap")]
pub fn make_stream_kind(info: FuturesTickerInfo) -> StreamKind {
    StreamKind::DepthAndTrades {
        ticker_info: info,
        depth_aggr: StreamTicksize::Client,
        push_freq: PushFrequency::ServerDefault,
    }
}
