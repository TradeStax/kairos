//! Rithmic Streaming
//!
//! Async receive loop that transforms RithmicMessage variants into
//! exchange::Event variants for the application layer.

use super::mapper;
use crate::adapter::{Event, StreamKind};
use rithmic_rs::RithmicTickerPlantHandle;
use rithmic_rs::rti::messages::RithmicMessage;
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

    /// Run the streaming loop, sending events to the provided channel
    ///
    /// This consumes self and runs until the connection is lost
    /// or the sender is dropped.
    pub async fn run(
        mut self,
        stream_kind: StreamKind,
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
                        RithmicMessage::LastTrade(_) => {
                            if let Some(trade) = mapper::map_last_trade(&response.message)
                                && event_tx
                                    .send(Event::TradeReceived(stream_kind, trade))
                                    .is_err()
                            {
                                log::info!(
                                    "Event channel closed, \
                                     stopping Rithmic stream"
                                );
                                break;
                            }
                        }
                        RithmicMessage::BestBidOffer(_) => {
                            if let Some((ts, depth)) =
                                mapper::map_bbo_to_exchange_depth(&response.message)
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
                        RithmicMessage::OrderBook(_) => {
                            if let Some((ts, depth)) =
                                mapper::map_orderbook_to_exchange_depth(&response.message)
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
                                "Rithmic stream: ignoring message type: {:?}",
                                std::mem::discriminant(other)
                            );
                        }
                    }
                }
                Err(e) => {
                    log::error!("Rithmic subscription receiver error: {}", e);
                    let _ = event_tx.send(Event::ConnectionLost);
                    break;
                }
            }
        }

        log::info!("Rithmic streaming loop ended");
    }
}
