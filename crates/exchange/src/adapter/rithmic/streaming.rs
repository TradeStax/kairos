//! Rithmic Streaming
//!
//! Async receive loop that transforms RithmicMessage variants into
//! exchange::Event variants for the application layer.

use super::mapper;
use super::plants::RithmicTickerPlantHandle;
use super::protocol::RithmicMessage;
use crate::FuturesTickerInfo;
use crate::adapter::stream::PushFrequency;
use crate::adapter::{Event, StreamKind, StreamTicksize};
use rustc_hash::FxHashMap;
use std::sync::Arc;

/// Rithmic streaming subscription
///
/// Wraps a ticker plant handle and produces exchange Events
/// from the subscription receiver.
pub struct RithmicStream {
    handle: RithmicTickerPlantHandle,
}

impl RithmicStream {
    pub fn new(handle: RithmicTickerPlantHandle) -> Self {
        Self { handle }
    }

    /// Run the streaming loop, sending events to the provided channel.
    ///
    /// `ticker_map` maps Rithmic symbol strings (e.g. "ES", "NQ") to
    /// their `FuturesTickerInfo` so each event is tagged with the
    /// correct tick size, point value, etc.
    ///
    /// This consumes self and runs until the connection is lost
    /// or the sender is dropped.
    pub async fn run(
        mut self,
        ticker_map: FxHashMap<String, FuturesTickerInfo>,
        event_tx: tokio::sync::mpsc::UnboundedSender<Event>,
    ) {
        log::info!("Rithmic streaming loop started");

        loop {
            match self.handle.subscription_receiver.recv().await {
                Ok(response) => {
                    if let Some(err) = &response.error {
                        log::warn!("Rithmic stream error: {}", err);
                        continue;
                    }

                    match &response.message {
                        RithmicMessage::LastTrade(lt) => {
                            let Some(stream_kind) =
                                resolve_stream_kind(
                                    lt.symbol.as_deref(),
                                    &ticker_map,
                                )
                            else {
                                continue;
                            };
                            if let Some(trade) =
                                mapper::map_last_trade(&response.message)
                                && event_tx
                                    .send(Event::TradeReceived(
                                        stream_kind, trade,
                                    ))
                                    .is_err()
                            {
                                log::info!(
                                    "Event channel closed, \
                                     stopping Rithmic stream"
                                );
                                break;
                            }
                        }
                        RithmicMessage::BestBidOffer(bbo) => {
                            let Some(stream_kind) =
                                resolve_stream_kind(
                                    bbo.symbol.as_deref(),
                                    &ticker_map,
                                )
                            else {
                                continue;
                            };
                            if let Some((ts, depth)) =
                                mapper::map_bbo_to_exchange_depth(
                                    &response.message,
                                )
                                && event_tx
                                    .send(Event::DepthReceived(
                                        stream_kind,
                                        ts,
                                        Arc::new(depth),
                                        Box::new([]),
                                    ))
                                    .is_err()
                            {
                                break;
                            }
                        }
                        RithmicMessage::OrderBook(ob) => {
                            let Some(stream_kind) =
                                resolve_stream_kind(
                                    ob.symbol.as_deref(),
                                    &ticker_map,
                                )
                            else {
                                continue;
                            };
                            if let Some((ts, depth)) =
                                mapper::map_orderbook_to_exchange_depth(
                                    &response.message,
                                )
                                && event_tx
                                    .send(Event::DepthReceived(
                                        stream_kind,
                                        ts,
                                        Arc::new(depth),
                                        Box::new([]),
                                    ))
                                    .is_err()
                            {
                                break;
                            }
                        }
                        RithmicMessage::ForcedLogout(_) => {
                            log::warn!("Rithmic forced logout");
                            let _ = event_tx.send(Event::ConnectionLost);
                            break;
                        }
                        RithmicMessage::ConnectionError => {
                            log::error!("Rithmic connection error");
                            let _ = event_tx.send(Event::ConnectionLost);
                            break;
                        }
                        RithmicMessage::HeartbeatTimeout => {
                            log::warn!("Rithmic heartbeat timeout");
                            let _ = event_tx.send(Event::ConnectionLost);
                            break;
                        }
                        other => {
                            log::trace!(
                                "Rithmic stream: ignoring message \
                                 type: {:?}",
                                std::mem::discriminant(other)
                            );
                        }
                    }
                }
                Err(e) => {
                    log::error!(
                        "Rithmic subscription receiver error: {}",
                        e
                    );
                    let _ = event_tx.send(Event::ConnectionLost);
                    break;
                }
            }
        }

        log::info!("Rithmic streaming loop ended");
    }
}

/// Resolve the correct `StreamKind` for a message's symbol.
///
/// Looks up the symbol in the ticker map. Falls back to the first
/// entry if no symbol is present on the message (shouldn't happen
/// in practice). Returns `None` only if the map is empty and no
/// symbol was provided.
fn resolve_stream_kind(
    symbol: Option<&str>,
    ticker_map: &FxHashMap<String, FuturesTickerInfo>,
) -> Option<StreamKind> {
    let info = if let Some(sym) = symbol {
        if let Some(info) = ticker_map.get(sym) {
            *info
        } else {
            // Try matching by product prefix (e.g. "ESH5" → "ES")
            let found = ticker_map
                .iter()
                .find(|(key, _)| sym.starts_with(key.as_str()));
            if let Some((_, info)) = found {
                *info
            } else {
                log::warn!(
                    "Unknown Rithmic symbol '{}', \
                     no matching ticker info",
                    sym
                );
                return None;
            }
        }
    } else {
        // No symbol on message — use first entry as fallback
        let Some(info) = ticker_map.values().next() else {
            return None;
        };
        *info
    };

    Some(StreamKind::DepthAndTrades {
        ticker_info: info,
        depth_aggr: StreamTicksize::Client,
        push_freq: PushFrequency::ServerDefault,
    })
}
