//! Rithmic Ticker Plant actor.
//!
//! Manages a WebSocket connection to Rithmic's Ticker Plant for
//! real-time market data: last trades, BBO quotes, depth-by-order
//! updates, symbol search, and reference data queries. Runs as a
//! background `tokio` task driven by a command channel.

use async_trait::async_trait;
use log::{error, info, warn};

use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::{
    net::TcpStream,
    sync::{broadcast, mpsc, oneshot},
    time::{Interval, sleep_until},
};
use tokio_tungstenite::{
    MaybeTlsStream,
    tungstenite::{Error, Message, error::ProtocolError},
};

use super::super::protocol::ws::ConnectStrategy;
use super::super::protocol::{
    config::RithmicConnectionConfig,
    messages::RithmicMessage,
    ping::PingManager,
    request::{RithmicRequest, RithmicRequestHandler},
    response::{RithmicReceiverApi, RithmicResponse},
    rti::{
        request_depth_by_order_updates,
        request_login::SysInfraType,
        request_market_data_update::{Request, UpdateBits},
        request_market_data_update_by_underlying, request_search_symbols,
    },
    sender::RithmicSenderApi,
    ws::{
        HEARTBEAT_SECS, PING_TIMEOUT_SECS, PlantActor, connect_with_strategy,
        get_heartbeat_interval, get_ping_interval,
    },
};

/// Commands sent to the ticker plant actor via its mpsc channel.
pub(crate) enum TickerPlantCommand {
    /// Gracefully close the WebSocket connection
    Close,
    /// Query available Rithmic system infrastructure
    ListSystemInfo {
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Authenticate with the ticker plant
    Login {
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Mark the actor as logged in (enables heartbeats)
    SetLogin,
    /// Log out and prepare to close
    Logout {
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Send a heartbeat to keep the session alive
    SendHeartbeat,
    /// Update the heartbeat interval
    UpdateHeartbeat { seconds: u64 },
    /// Subscribe to or unsubscribe from market data updates
    Subscribe {
        symbol: String,
        exchange: String,
        fields: Vec<UpdateBits>,
        request_type: Request,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Subscribe to or unsubscribe from depth-by-order updates
    SubscribeOrderBook {
        symbol: String,
        exchange: String,
        request_type: request_depth_by_order_updates::Request,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Request a one-time depth-by-order snapshot
    RequestDepthByOrderSnapshot {
        symbol: String,
        exchange: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Search for symbols matching criteria
    SearchSymbols {
        search_text: String,
        exchange: Option<String>,
        product_code: Option<String>,
        instrument_type: Option<request_search_symbols::InstrumentType>,
        pattern: Option<request_search_symbols::Pattern>,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// List available exchanges for a user
    ListExchanges {
        user: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Get instruments for an underlying symbol
    GetInstrumentByUnderlying {
        underlying_symbol: String,
        exchange: String,
        expiration_date: Option<String>,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Subscribe to market data by underlying symbol
    SubscribeByUnderlying {
        underlying_symbol: String,
        exchange: String,
        expiration_date: Option<String>,
        fields: Vec<request_market_data_update_by_underlying::UpdateBits>,
        request_type: request_market_data_update_by_underlying::Request,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Request tick size type table
    GetTickSizeTypeTable {
        tick_size_type: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Request available product codes
    GetProductCodes {
        exchange: Option<String>,
        give_toi_products_only: Option<bool>,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Request volume-at-price data
    GetVolumeAtPrice {
        symbol: String,
        exchange: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Request auxiliary reference data
    GetAuxilliaryReferenceData {
        symbol: String,
        exchange: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Request instrument reference data
    GetReferenceData {
        symbol: String,
        exchange: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Request front-month contract information
    GetFrontMonthContract {
        symbol: String,
        exchange: String,
        need_updates: bool,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Request system gateway information
    GetSystemGatewayInfo {
        system_name: Option<String>,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
}

/// Owns a ticker plant WebSocket actor task and its communication channels.
///
/// Supports last trades, BBO quotes, and depth-by-order updates.
/// Connection health is monitored via WebSocket ping/pong (primary)
/// and application-level heartbeats (secondary). Successful heartbeat
/// responses are silently dropped; only errors are surfaced as
/// `HeartbeatTimeout` messages.
#[allow(dead_code)]
pub struct RithmicTickerPlant {
    /// Background task running the plant actor loop
    pub connection_handle: tokio::task::JoinHandle<()>,
    /// Command channel sender
    sender: mpsc::Sender<TickerPlantCommand>,
    /// Broadcast sender for subscription updates
    subscription_sender: broadcast::Sender<RithmicResponse>,
}

impl RithmicTickerPlant {
    /// Connects to Rithmic's Ticker Plant and spawns the actor task.
    ///
    /// The plant is not yet authenticated -- call
    /// [`RithmicTickerPlantHandle::login`] on the returned handle
    /// before subscribing to market data.
    pub async fn connect(
        config: &RithmicConnectionConfig,
        strategy: ConnectStrategy,
    ) -> Result<RithmicTickerPlant, Box<dyn std::error::Error>> {
        let (req_tx, req_rx) = mpsc::channel::<TickerPlantCommand>(64);
        let (sub_tx, _sub_rx) = broadcast::channel(10_000);

        let mut ticker_plant = TickerPlant::new(req_rx, sub_tx.clone(), config, strategy).await?;

        let connection_handle = tokio::spawn(async move {
            ticker_plant.run().await;
        });

        Ok(RithmicTickerPlant {
            connection_handle,
            sender: req_tx,
            subscription_sender: sub_tx,
        })
    }
}

impl RithmicTickerPlant {
    /// Returns a cloneable handle for sending commands to this plant
    pub fn get_handle(&self) -> RithmicTickerPlantHandle {
        RithmicTickerPlantHandle {
            sender: self.sender.clone(),
            subscription_sender: self.subscription_sender.clone(),
            subscription_receiver: self.subscription_sender.subscribe(),
        }
    }
}

/// Internal actor state for a ticker plant WebSocket connection.
#[derive(Debug)]
struct TickerPlant {
    config: RithmicConnectionConfig,
    interval: Interval,
    logged_in: bool,
    ping_interval: Interval,
    ping_manager: PingManager,
    request_handler: RithmicRequestHandler,
    request_receiver: mpsc::Receiver<TickerPlantCommand>,
    rithmic_reader: SplitStream<tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>>,
    rithmic_receiver_api: RithmicReceiverApi,
    rithmic_sender: SplitSink<
        tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>,
        tokio_tungstenite::tungstenite::Message,
    >,
    rithmic_sender_api: RithmicSenderApi,
    subscription_sender: broadcast::Sender<RithmicResponse>,
    /// Counter for broadcast market data messages (diagnostics)
    broadcast_count: u64,
    /// When the last stats log was emitted
    last_stats_log: std::time::Instant,
}

impl TickerPlant {
    async fn new(
        request_receiver: mpsc::Receiver<TickerPlantCommand>,
        subscription_sender: broadcast::Sender<RithmicResponse>,
        config: &RithmicConnectionConfig,
        strategy: ConnectStrategy,
    ) -> Result<TickerPlant, Box<dyn std::error::Error>> {
        let ws_stream = connect_with_strategy(&config.url, &config.beta_url, strategy).await?;

        let (rithmic_sender, rithmic_reader) = ws_stream.split();

        let rithmic_sender_api = RithmicSenderApi::new(config);
        let rithmic_receiver_api = RithmicReceiverApi {
            source: "ticker_plant".to_string(),
        };

        let interval = get_heartbeat_interval(None);
        let ping_interval = get_ping_interval(None);
        let ping_manager = PingManager::new(PING_TIMEOUT_SECS);

        Ok(TickerPlant {
            config: config.clone(),
            interval,
            ping_interval,
            logged_in: false,
            ping_manager,
            request_handler: RithmicRequestHandler::new(),
            request_receiver,
            rithmic_reader,
            rithmic_receiver_api,
            rithmic_sender_api,
            rithmic_sender,
            subscription_sender,
            broadcast_count: 0,
            last_stats_log: std::time::Instant::now(),
        })
    }
}

#[async_trait]
impl PlantActor for TickerPlant {
    type Command = TickerPlantCommand;

    /// Execute the ticker plant in its own thread
    /// We will listen for messages from request_receiver and forward them
    /// to Rithmic while also listening for messages from Rithmic and
    /// forwarding them to subscription_sender or request handler
    async fn run(&mut self) {
        loop {
            tokio::select! {
                _ = self.interval.tick() => {
                    if self.logged_in {
                        self.handle_command(
                            TickerPlantCommand::SendHeartbeat,
                        ).await;
                    }
                }
                _ = self.ping_interval.tick() => {
                    self.ping_manager.sent();
                    let _ = self.rithmic_sender
                        .send(Message::Ping(vec![].into()))
                        .await;
                }
                _ = async {
                    if let Some(timeout_at) =
                        self.ping_manager.next_timeout_at()
                    {
                        sleep_until(timeout_at).await
                    } else {
                        std::future::pending::<()>().await
                    }
                } => {
                    if self.ping_manager.check_timeout() {
                        error!(
                            "WebSocket ping timed out \
                             - connection appears dead"
                        );

                        let error_response = RithmicResponse {
                            request_id:
                                "websocket_ping_timeout".to_string(),
                            message: RithmicMessage::HeartbeatTimeout,
                            is_update: true,
                            has_more: false,
                            multi_response: false,
                            error: Some(
                                "WebSocket ping timeout \
                                 - connection dead"
                                    .to_string(),
                            ),
                            source: self
                                .rithmic_receiver_api
                                .source
                                .clone(),
                        };
                        let _ = self
                            .subscription_sender
                            .send(error_response);
                        break;
                    }
                }
                Some(message) = self.request_receiver.recv() => {
                    self.handle_command(message).await;
                }
                Some(message) = self.rithmic_reader.next() => {
                    let stop = self
                        .handle_rithmic_message(message)
                        .await
                        .unwrap();

                    if stop {
                        break;
                    }
                }
                else => { break }
            }
        }
    }

    async fn handle_rithmic_message(
        &mut self,
        message: Result<Message, Error>,
    ) -> Result<bool, ()> {
        let mut stop = false;

        match message {
            Ok(Message::Close(frame)) => {
                info!("ticker_plant received close frame: {:?}", frame);

                stop = true;
            }
            Ok(Message::Pong(_)) => {
                self.ping_manager.received();
            }
            Ok(Message::Binary(data)) => {
                match self.rithmic_receiver_api.buf_to_message(data) {
                    Ok(response) => {
                        // Handle heartbeat responses: only forward
                        // if they contain an error
                        if matches!(response.message, RithmicMessage::ResponseHeartbeat(_)) {
                            if let Some(error) = response.error {
                                let error_response = RithmicResponse {
                                    request_id: response.request_id,
                                    message: RithmicMessage::HeartbeatTimeout,
                                    is_update: true,
                                    has_more: false,
                                    multi_response: false,
                                    error: Some(error),
                                    source: self.rithmic_receiver_api.source.clone(),
                                };

                                let _ = self.subscription_sender.send(error_response);
                            }

                            // Always drop heartbeat responses
                            // (successful or error)
                            return Ok(false);
                        }

                        if response.is_update {
                            let is_market = response.is_market_data();
                            match self.subscription_sender.send(response) {
                                Ok(n) => {
                                    if is_market {
                                        self.broadcast_count += 1;
                                        if self.broadcast_count == 1 {
                                            info!(
                                                "ticker_plant: first \
                                                 market data broadcast \
                                                 ({} receiver(s))",
                                                n
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(
                                        "ticker_plant: \
                                         no active subscribers: {:?}",
                                        e
                                    );
                                }
                            }
                            // Periodic stats
                            if self.last_stats_log.elapsed() >= std::time::Duration::from_secs(60) {
                                info!(
                                    "ticker_plant: {} market data \
                                     messages broadcast",
                                    self.broadcast_count
                                );
                                self.last_stats_log = std::time::Instant::now();
                            }
                        } else {
                            self.request_handler.handle_response(response);
                        }
                    }
                    Err(err_response) => {
                        error!(
                            "ticker_plant: error response \
                             from server: {:?}",
                            err_response
                        );

                        if err_response.is_update {
                            let _ = self.subscription_sender.send(err_response);
                        } else {
                            self.request_handler.handle_response(err_response);
                        }
                    }
                }
            }
            Err(Error::ConnectionClosed) => {
                error!("ticker_plant: connection closed");

                let error_response = RithmicResponse {
                    request_id: "".to_string(),
                    message: RithmicMessage::ConnectionError,
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some("WebSocket connection closed".to_string()),
                    source: self.rithmic_receiver_api.source.clone(),
                };
                let _ = self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::AlreadyClosed) => {
                error!("ticker_plant: connection already closed");

                let error_response = RithmicResponse {
                    request_id: "".to_string(),
                    message: RithmicMessage::ConnectionError,
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some("WebSocket connection already closed".to_string()),
                    source: self.rithmic_receiver_api.source.clone(),
                };
                let _ = self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::Io(ref io_err)) => {
                error!("ticker_plant: I/O error: {}", io_err);

                let error_response = RithmicResponse {
                    request_id: "".to_string(),
                    message: RithmicMessage::ConnectionError,
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some(format!("WebSocket I/O error: {}", io_err)),
                    source: self.rithmic_receiver_api.source.clone(),
                };
                let _ = self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::Protocol(ProtocolError::ResetWithoutClosingHandshake)) => {
                error!(
                    "ticker_plant: connection reset \
                     without closing handshake"
                );

                let error_response = RithmicResponse {
                    request_id: "".to_string(),
                    message: RithmicMessage::ConnectionError,
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some(
                        "WebSocket connection reset \
                         without closing handshake"
                            .to_string(),
                    ),
                    source: self.rithmic_receiver_api.source.clone(),
                };
                let _ = self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::Protocol(ProtocolError::SendAfterClosing)) => {
                error!("ticker_plant: attempted to send after closing");

                let error_response = RithmicResponse {
                    request_id: "".to_string(),
                    message: RithmicMessage::ConnectionError,
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some("WebSocket attempted to send after closing".to_string()),
                    source: self.rithmic_receiver_api.source.clone(),
                };
                let _ = self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::Protocol(ProtocolError::ReceivedAfterClosing)) => {
                error!("ticker_plant: received data after closing");

                let error_response = RithmicResponse {
                    request_id: "".to_string(),
                    message: RithmicMessage::ConnectionError,
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some("WebSocket received data after closing".to_string()),
                    source: self.rithmic_receiver_api.source.clone(),
                };
                let _ = self.subscription_sender.send(error_response);

                stop = true;
            }
            _ => {
                warn!("ticker_plant received unknown message {:?}", message);
            }
        }

        Ok(stop)
    }

    async fn handle_command(&mut self, command: TickerPlantCommand) {
        match command {
            TickerPlantCommand::Close => {
                self.rithmic_sender
                    .send(Message::Close(None))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::ListSystemInfo { response_sender } => {
                let (list_system_info_buf, id) =
                    self.rithmic_sender_api.request_rithmic_system_info();

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(list_system_info_buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::Login { response_sender } => {
                let (login_buf, id) = self.rithmic_sender_api.request_login(
                    &self.config.system_name,
                    SysInfraType::TickerPlant,
                    &self.config.user,
                    &self.config.password,
                );

                info!("ticker_plant: sending login request {}", id);

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(login_buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::SetLogin => {
                self.logged_in = true;
            }
            TickerPlantCommand::Logout { response_sender } => {
                let (logout_buf, id) = self.rithmic_sender_api.request_logout();

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(logout_buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::SendHeartbeat => {
                let (heartbeat_buf, _id) = self.rithmic_sender_api.request_heartbeat();

                let _ = self
                    .rithmic_sender
                    .send(Message::Binary(heartbeat_buf.into()))
                    .await;
            }
            TickerPlantCommand::UpdateHeartbeat { seconds } => {
                self.interval = get_heartbeat_interval(Some(seconds));
            }
            TickerPlantCommand::Subscribe {
                symbol,
                exchange,
                fields,
                request_type,
                response_sender,
            } => {
                let (sub_buf, id) = self.rithmic_sender_api.request_market_data_update(
                    &symbol,
                    &exchange,
                    fields,
                    request_type,
                );

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(sub_buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::SubscribeOrderBook {
                symbol,
                exchange,
                request_type,
                response_sender,
            } => {
                let (sub_buf, id) = self.rithmic_sender_api.request_depth_by_order_update(
                    &symbol,
                    &exchange,
                    request_type,
                );

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(sub_buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::RequestDepthByOrderSnapshot {
                symbol,
                exchange,
                response_sender,
            } => {
                let (snapshot_buf, id) = self
                    .rithmic_sender_api
                    .request_depth_by_order_snapshot(&symbol, &exchange);

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(snapshot_buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::SearchSymbols {
                search_text,
                exchange,
                product_code,
                instrument_type,
                pattern,
                response_sender,
            } => {
                let (search_buf, id) = self.rithmic_sender_api.request_search_symbols(
                    &search_text,
                    exchange.as_deref(),
                    product_code.as_deref(),
                    instrument_type,
                    pattern,
                );

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(search_buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::ListExchanges {
                user,
                response_sender,
            } => {
                let (list_buf, id) = self.rithmic_sender_api.request_list_exchanges(&user);

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(list_buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::GetInstrumentByUnderlying {
                underlying_symbol,
                exchange,
                expiration_date,
                response_sender,
            } => {
                let (buf, id) = self
                    .rithmic_sender_api
                    .request_get_instrument_by_underlying(
                        &underlying_symbol,
                        &exchange,
                        expiration_date.as_deref(),
                    );

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::SubscribeByUnderlying {
                underlying_symbol,
                exchange,
                expiration_date,
                fields,
                request_type,
                response_sender,
            } => {
                let (buf, id) = self
                    .rithmic_sender_api
                    .request_market_data_update_by_underlying(
                        &underlying_symbol,
                        &exchange,
                        expiration_date.as_deref(),
                        fields,
                        request_type,
                    );

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::GetTickSizeTypeTable {
                tick_size_type,
                response_sender,
            } => {
                let (buf, id) = self
                    .rithmic_sender_api
                    .request_give_tick_size_type_table(&tick_size_type);

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::GetProductCodes {
                exchange,
                give_toi_products_only,
                response_sender,
            } => {
                let (buf, id) = self
                    .rithmic_sender_api
                    .request_product_codes(exchange.as_deref(), give_toi_products_only);

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::GetVolumeAtPrice {
                symbol,
                exchange,
                response_sender,
            } => {
                let (buf, id) = self
                    .rithmic_sender_api
                    .request_get_volume_at_price(&symbol, &exchange);

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::GetAuxilliaryReferenceData {
                symbol,
                exchange,
                response_sender,
            } => {
                let (buf, id) = self
                    .rithmic_sender_api
                    .request_auxilliary_reference_data(&symbol, &exchange);

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::GetReferenceData {
                symbol,
                exchange,
                response_sender,
            } => {
                let (buf, id) = self
                    .rithmic_sender_api
                    .request_reference_data(&symbol, &exchange);

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::GetFrontMonthContract {
                symbol,
                exchange,
                need_updates,
                response_sender,
            } => {
                let (buf, id) = self.rithmic_sender_api.request_front_month_contract(
                    &symbol,
                    &exchange,
                    need_updates,
                );

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::GetSystemGatewayInfo {
                system_name,
                response_sender,
            } => {
                let (buf, id) = self
                    .rithmic_sender_api
                    .request_rithmic_system_gateway_info(system_name.as_deref());

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                self.rithmic_sender
                    .send(Message::Binary(buf.into()))
                    .await
                    .unwrap();
            }
        }
    }
}

/// Cloneable handle for interacting with a ticker plant actor.
///
/// Provides async methods for login, market data subscription,
/// symbol search, and reference data queries. Each method sends a
/// command to the actor task and awaits the response.
pub struct RithmicTickerPlantHandle {
    /// Command channel to the actor task
    sender: mpsc::Sender<TickerPlantCommand>,
    /// Used to create new broadcast receivers on clone
    subscription_sender: broadcast::Sender<RithmicResponse>,
    /// Broadcast receiver for subscription updates
    pub subscription_receiver: broadcast::Receiver<RithmicResponse>,
}

impl RithmicTickerPlantHandle {
    /// Lists available Rithmic system infrastructure information
    pub async fn list_system_info(&self) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::ListSystemInfo {
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;
        let response = rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0);

        Ok(response)
    }

    /// Logs in to the Rithmic Ticker Plant.
    ///
    /// Must be called before subscribing to market data. Configures
    /// the heartbeat interval from the server's login response.
    pub async fn login(&self) -> Result<RithmicResponse, String> {
        info!("ticker_plant: logging in");

        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::Login {
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;
        let response = rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0);

        if let Some(err) = response.error {
            error!("ticker_plant: login failed {:?}", err);
            Err(err)
        } else {
            let _ = self.sender.send(TickerPlantCommand::SetLogin).await;

            if let RithmicMessage::ResponseLogin(resp) = &response.message {
                if let Some(hb) = resp.heartbeat_interval {
                    let secs = hb.max(HEARTBEAT_SECS as f64) as u64;
                    self.update_heartbeat(secs).await;
                }

                if let Some(session_id) = &resp.unique_user_id {
                    info!("ticker_plant: session id: {}", session_id);
                }
            }

            info!("ticker_plant: logged in");

            Ok(response)
        }
    }

    /// Disconnects from the Rithmic Ticker Plant (logout + close)
    pub async fn disconnect(&self) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::Logout {
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;
        let mut r = rx.await.map_err(|_| "Connection closed".to_string())??;
        let _ = self.sender.send(TickerPlantCommand::Close).await;
        let response = r.remove(0);

        self.subscription_sender.send(response.clone()).unwrap();

        Ok(response)
    }

    /// Subscribes to last-trade and BBO updates for a symbol
    pub async fn subscribe(&self, symbol: &str, exchange: &str) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::Subscribe {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            fields: vec![UpdateBits::LastTrade, UpdateBits::Bbo],
            request_type: Request::Subscribe,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        Ok(rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0))
    }

    /// Subscribes to depth-by-order updates for a symbol
    pub async fn subscribe_order_book(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::SubscribeOrderBook {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            request_type: request_depth_by_order_updates::Request::Subscribe,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        Ok(rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0))
    }

    /// Unsubscribes from market data for a symbol
    pub async fn unsubscribe(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::Subscribe {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            fields: vec![UpdateBits::LastTrade, UpdateBits::Bbo],
            request_type: Request::Unsubscribe,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        Ok(rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0))
    }

    /// Unsubscribes from depth-by-order updates for a symbol
    pub async fn unsubscribe_order_book(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::SubscribeOrderBook {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            request_type: request_depth_by_order_updates::Request::Unsubscribe,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        Ok(rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0))
    }

    /// Requests a one-time depth-by-order snapshot for a symbol
    pub async fn request_depth_by_order_snapshot(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::RequestDepthByOrderSnapshot {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Sends a command to update the heartbeat interval
    async fn update_heartbeat(&self, seconds: u64) {
        let command = TickerPlantCommand::UpdateHeartbeat { seconds };

        let _ = self.sender.send(command).await;
    }

    /// Searches for symbols matching the given criteria
    pub async fn search_symbols(
        &self,
        search_text: &str,
        exchange: Option<&str>,
        product_code: Option<&str>,
        instrument_type: Option<request_search_symbols::InstrumentType>,
        pattern: Option<request_search_symbols::Pattern>,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::SearchSymbols {
            search_text: search_text.to_string(),
            exchange: exchange.map(|e| e.to_string()),
            product_code: product_code.map(|p| p.to_string()),
            instrument_type,
            pattern,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Lists exchanges available to the specified user
    pub async fn list_exchanges(&self, user: &str) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::ListExchanges {
            user: user.to_string(),
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Returns instruments for an underlying symbol
    pub async fn get_instrument_by_underlying(
        &self,
        underlying_symbol: &str,
        exchange: &str,
        expiration_date: Option<&str>,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::GetInstrumentByUnderlying {
            underlying_symbol: underlying_symbol.to_string(),
            exchange: exchange.to_string(),
            expiration_date: expiration_date.map(|d| d.to_string()),
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Subscribes to market data for all instruments of an underlying
    pub async fn subscribe_by_underlying(
        &self,
        underlying_symbol: &str,
        exchange: &str,
        expiration_date: Option<&str>,
        fields: Vec<request_market_data_update_by_underlying::UpdateBits>,
        request_type: request_market_data_update_by_underlying::Request,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::SubscribeByUnderlying {
            underlying_symbol: underlying_symbol.to_string(),
            exchange: exchange.to_string(),
            expiration_date: expiration_date.map(|d| d.to_string()),
            fields,
            request_type,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        Ok(rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0))
    }

    /// Returns the tick size type table for a given type identifier
    pub async fn get_tick_size_type_table(
        &self,
        tick_size_type: &str,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::GetTickSizeTypeTable {
            tick_size_type: tick_size_type.to_string(),
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Returns available product codes, optionally filtered by exchange
    pub async fn get_product_codes(
        &self,
        exchange: Option<&str>,
        give_toi_products_only: Option<bool>,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::GetProductCodes {
            exchange: exchange.map(|e| e.to_string()),
            give_toi_products_only,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Returns volume-at-price data for a symbol
    pub async fn get_volume_at_price(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::GetVolumeAtPrice {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Returns auxiliary reference data for a symbol
    pub async fn get_auxilliary_reference_data(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::GetAuxilliaryReferenceData {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        Ok(rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0))
    }

    /// Returns instrument reference data (tick size, point value, etc.)
    pub async fn get_reference_data(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::GetReferenceData {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        Ok(rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0))
    }

    /// Returns the current front-month contract for a product
    pub async fn get_front_month_contract(
        &self,
        symbol: &str,
        exchange: &str,
        need_updates: bool,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::GetFrontMonthContract {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            need_updates,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        Ok(rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0))
    }

    /// Returns Rithmic system gateway information
    pub async fn get_system_gateway_info(
        &self,
        system_name: Option<&str>,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::GetSystemGatewayInfo {
            system_name: system_name.map(|s| s.to_string()),
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        Ok(rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0))
    }
}

impl Clone for RithmicTickerPlantHandle {
    fn clone(&self) -> Self {
        RithmicTickerPlantHandle {
            sender: self.sender.clone(),
            subscription_receiver: self.subscription_sender.subscribe(),
            subscription_sender: self.subscription_sender.clone(),
        }
    }
}
