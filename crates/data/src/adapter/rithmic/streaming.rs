//! Rithmic Streaming
//!
//! Async receive loop that transforms RithmicMessage variants into
//! DataEvents for the application layer.

use super::mapper;
use super::plants::RithmicTickerPlantHandle;
use super::protocol::RithmicMessage;
use crate::domain::{FuturesTicker, FuturesTickerInfo};
use crate::event::DataEvent;
#[cfg(feature = "heatmap")]
use crate::stream::kind::{PushFrequency, StreamKind, StreamTicksize};
use rustc_hash::FxHashMap;

/// Rithmic streaming subscription
///
/// Wraps a ticker plant handle and produces DataEvents
/// from the subscription receiver.
pub struct RithmicStream {
    handle: RithmicTickerPlantHandle,
}

impl RithmicStream {
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
                            let Some(ticker) = resolve_ticker(lt.symbol.as_deref(), &ticker_map)
                            else {
                                continue;
                            };
                            if let Some(trade) = mapper::map_last_trade(&response.message)
                                && event_tx
                                    .send(DataEvent::TradeReceived { ticker, trade })
                                    .is_err()
                            {
                                log::info!("Event channel closed, stopping Rithmic stream");
                                break;
                            }
                        }
                        #[cfg(feature = "heatmap")]
                        RithmicMessage::BestBidOffer(bbo) => {
                            let Some(ticker) = resolve_ticker(bbo.symbol.as_deref(), &ticker_map)
                            else {
                                continue;
                            };
                            if let Some(depth) = mapper::map_bbo_to_depth(&response.message)
                                && event_tx
                                    .send(DataEvent::DepthReceived { ticker, depth })
                                    .is_err()
                            {
                                break;
                            }
                        }
                        #[cfg(feature = "heatmap")]
                        RithmicMessage::OrderBook(ob) => {
                            let Some(ticker) = resolve_ticker(ob.symbol.as_deref(), &ticker_map)
                            else {
                                continue;
                            };
                            if let Some(depth) = mapper::map_orderbook_to_depth(&response.message)
                                && event_tx
                                    .send(DataEvent::DepthReceived { ticker, depth })
                                    .is_err()
                            {
                                break;
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
                        other => {
                            log::trace!(
                                "Rithmic stream: ignoring {:?}",
                                std::mem::discriminant(other)
                            );
                        }
                    }
                }
                Err(e) => {
                    log::error!("Rithmic subscription receiver error: {}", e);
                    break;
                }
            }
        }

        log::info!("Rithmic streaming loop ended");
    }
}

/// Resolve the ticker for a message's symbol.
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

/// Build a StreamKind for a Rithmic subscription (for compatibility)
#[cfg(feature = "heatmap")]
pub fn make_stream_kind(info: FuturesTickerInfo) -> StreamKind {
    StreamKind::DepthAndTrades {
        ticker_info: info,
        depth_aggr: StreamTicksize::Client,
        push_freq: PushFrequency::ServerDefault,
    }
}
