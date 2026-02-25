use async_trait::async_trait;
use log::{error, info, warn};

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

use super::super::protocol::ws::ConnectStrategy;

use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};

use tokio_tungstenite::{
    MaybeTlsStream,
    tungstenite::{Error, Message, error::ProtocolError},
};

use tokio::{
    net::TcpStream,
    sync::{broadcast, mpsc, oneshot},
    time::{Interval, sleep_until},
};

pub(crate) enum TickerPlantCommand {
    Close,
    ListSystemInfo {
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    Login {
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    SetLogin,
    Logout {
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    SendHeartbeat,
    UpdateHeartbeat {
        seconds: u64,
    },
    Subscribe {
        symbol: String,
        exchange: String,
        fields: Vec<UpdateBits>,
        request_type: Request,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    SubscribeOrderBook {
        symbol: String,
        exchange: String,
        request_type: request_depth_by_order_updates::Request,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    RequestDepthByOrderSnapshot {
        symbol: String,
        exchange: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    SearchSymbols {
        search_text: String,
        exchange: Option<String>,
        product_code: Option<String>,
        instrument_type: Option<request_search_symbols::InstrumentType>,
        pattern: Option<request_search_symbols::Pattern>,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    ListExchanges {
        user: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    GetInstrumentByUnderlying {
        underlying_symbol: String,
        exchange: String,
        expiration_date: Option<String>,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    SubscribeByUnderlying {
        underlying_symbol: String,
        exchange: String,
        expiration_date: Option<String>,
        fields: Vec<request_market_data_update_by_underlying::UpdateBits>,
        request_type: request_market_data_update_by_underlying::Request,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    GetTickSizeTypeTable {
        tick_size_type: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    GetProductCodes {
        exchange: Option<String>,
        give_toi_products_only: Option<bool>,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    GetVolumeAtPrice {
        symbol: String,
        exchange: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    GetAuxilliaryReferenceData {
        symbol: String,
        exchange: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    GetReferenceData {
        symbol: String,
        exchange: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    GetFrontMonthContract {
        symbol: String,
        exchange: String,
        need_updates: bool,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    GetSystemGatewayInfo {
        system_name: Option<String>,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
}

/// The RithmicTickerPlant provides access to real-time market data.
///
/// Currently the following market data updates are supported:
/// - Last trades
/// - Best bid and offer (BBO)
/// - Order book depth-by-order updates
///
/// # Connection Health Monitoring
///
/// The subscription receiver provides connection health events:
/// - **WebSocket ping/pong timeouts**: Primary indicator of dead connections (auto-detected)
/// - **Heartbeat errors**: Only forwarded when Rithmic server returns an error (rare)
/// - **Forced logout events**: Server-initiated disconnections requiring reconnection
/// - **Market data updates**: Real-time trade and quote data
///
/// **Note:** Heartbeat requests are sent automatically to comply with Rithmic's protocol,
/// but successful responses are silently dropped. Only heartbeat errors from the server
/// are forwarded as `HeartbeatTimeout` messages. The primary keep-alive mechanism is
/// WebSocket ping/pong, which reliably detects dead connections 24/7.
///
pub struct RithmicTickerPlant {
    pub connection_handle: tokio::task::JoinHandle<()>,
    sender: mpsc::Sender<TickerPlantCommand>,
    subscription_sender: broadcast::Sender<RithmicResponse>,
}

impl RithmicTickerPlant {
    /// Connect to the Rithmic Ticker Plant to access real-time market data.
    ///
    /// # Arguments
    /// * `config` - Rithmic configuration with credentials and server URLs
    /// * `strategy` - Connection strategy (Simple or AlternateWithRetry)
    ///
    /// # Returns
    /// A `Result` containing the connected `RithmicTickerPlant` instance,
    /// or an error if the connection fails.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Unable to establish WebSocket connection to the server
    /// - Network timeout occurs
    /// - Server rejects the connection
    pub async fn connect(
        config: &RithmicConnectionConfig,
        strategy: ConnectStrategy,
    ) -> Result<RithmicTickerPlant, Box<dyn std::error::Error>> {
        let (req_tx, req_rx) = mpsc::channel::<TickerPlantCommand>(64);
        let (sub_tx, _sub_rx) = broadcast::channel(10_000);

        let mut ticker_plant =
            TickerPlant::new(req_rx, sub_tx.clone(), config, strategy).await?;

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
    /// Get a handle to interact with the ticker plant.
    ///
    /// The handle provides methods to subscribe to market data and receive updates.
    /// Multiple handles can be created from the same plant.
    pub fn get_handle(&self) -> RithmicTickerPlantHandle {
        RithmicTickerPlantHandle {
            sender: self.sender.clone(),
            subscription_sender: self.subscription_sender.clone(),
            subscription_receiver: self.subscription_sender.subscribe(),
        }
    }
}

#[derive(Debug)]
struct TickerPlant {
    config: RithmicConnectionConfig,
    interval: Interval,
    logged_in: bool,
    ping_interval: Interval,
    ping_manager: PingManager,
    request_handler: RithmicRequestHandler,
    request_receiver: mpsc::Receiver<TickerPlantCommand>,
    rithmic_reader:
        SplitStream<tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>>,
    rithmic_receiver_api: RithmicReceiverApi,
    rithmic_sender: SplitSink<
        tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>,
        tokio_tungstenite::tungstenite::Message,
    >,

    rithmic_sender_api: RithmicSenderApi,
    subscription_sender: broadcast::Sender<RithmicResponse>,
}

impl TickerPlant {
    async fn new(
        request_receiver: mpsc::Receiver<TickerPlantCommand>,
        subscription_sender: broadcast::Sender<RithmicResponse>,
        config: &RithmicConnectionConfig,
        strategy: ConnectStrategy,
    ) -> Result<TickerPlant, Box<dyn std::error::Error>> {
        let ws_stream =
            connect_with_strategy(&config.url, &config.beta_url, strategy).await?;

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
                info!(
                    "ticker_plant received close frame: {:?}",
                    frame
                );

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
                        if matches!(
                            response.message,
                            RithmicMessage::ResponseHeartbeat(_)
                        ) {
                            if let Some(error) = response.error {
                                let error_response = RithmicResponse {
                                    request_id: response.request_id,
                                    message:
                                        RithmicMessage::HeartbeatTimeout,
                                    is_update: true,
                                    has_more: false,
                                    multi_response: false,
                                    error: Some(error),
                                    source: self
                                        .rithmic_receiver_api
                                        .source
                                        .clone(),
                                };

                                let _ = self
                                    .subscription_sender
                                    .send(error_response);
                            }

                            // Always drop heartbeat responses
                            // (successful or error)
                            return Ok(false);
                        }

                        if response.is_update {
                            match self
                                .subscription_sender
                                .send(response)
                            {
                                Ok(_) => {}
                                Err(e) => {
                                    warn!(
                                        "ticker_plant: \
                                         no active subscribers: {:?}",
                                        e
                                    );
                                }
                            }
                        } else {
                            self.request_handler
                                .handle_response(response);
                        }
                    }
                    Err(err_response) => {
                        error!(
                            "ticker_plant: error response \
                             from server: {:?}",
                            err_response
                        );

                        if err_response.is_update {
                            let _ = self
                                .subscription_sender
                                .send(err_response);
                        } else {
                            self.request_handler
                                .handle_response(err_response);
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
                    error: Some(
                        "WebSocket connection closed".to_string(),
                    ),
                    source: self
                        .rithmic_receiver_api
                        .source
                        .clone(),
                };
                let _ =
                    self.subscription_sender.send(error_response);

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
                    error: Some(
                        "WebSocket connection already closed"
                            .to_string(),
                    ),
                    source: self
                        .rithmic_receiver_api
                        .source
                        .clone(),
                };
                let _ =
                    self.subscription_sender.send(error_response);

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
                    error: Some(format!(
                        "WebSocket I/O error: {}",
                        io_err
                    )),
                    source: self
                        .rithmic_receiver_api
                        .source
                        .clone(),
                };
                let _ =
                    self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::Protocol(
                ProtocolError::ResetWithoutClosingHandshake,
            )) => {
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
                    source: self
                        .rithmic_receiver_api
                        .source
                        .clone(),
                };
                let _ =
                    self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::Protocol(
                ProtocolError::SendAfterClosing,
            )) => {
                error!(
                    "ticker_plant: attempted to send after closing"
                );

                let error_response = RithmicResponse {
                    request_id: "".to_string(),
                    message: RithmicMessage::ConnectionError,
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some(
                        "WebSocket attempted to send after closing"
                            .to_string(),
                    ),
                    source: self
                        .rithmic_receiver_api
                        .source
                        .clone(),
                };
                let _ =
                    self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::Protocol(
                ProtocolError::ReceivedAfterClosing,
            )) => {
                error!(
                    "ticker_plant: received data after closing"
                );

                let error_response = RithmicResponse {
                    request_id: "".to_string(),
                    message: RithmicMessage::ConnectionError,
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some(
                        "WebSocket received data after closing"
                            .to_string(),
                    ),
                    source: self
                        .rithmic_receiver_api
                        .source
                        .clone(),
                };
                let _ =
                    self.subscription_sender.send(error_response);

                stop = true;
            }
            _ => {
                warn!(
                    "ticker_plant received unknown message {:?}",
                    message
                );
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
            TickerPlantCommand::ListSystemInfo {
                response_sender,
            } => {
                let (list_system_info_buf, id) =
                    self.rithmic_sender_api.request_rithmic_system_info();

                self.request_handler
                    .register_request(RithmicRequest {
                        request_id: id,
                        responder: response_sender,
                    });

                self.rithmic_sender
                    .send(Message::Binary(
                        list_system_info_buf.into(),
                    ))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::Login { response_sender } => {
                let (login_buf, id) =
                    self.rithmic_sender_api.request_login(
                        &self.config.system_name,
                        SysInfraType::TickerPlant,
                        &self.config.user,
                        &self.config.password,
                    );

                info!(
                    "ticker_plant: sending login request {}",
                    id
                );

                self.request_handler
                    .register_request(RithmicRequest {
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
                let (logout_buf, id) =
                    self.rithmic_sender_api.request_logout();

                self.request_handler
                    .register_request(RithmicRequest {
                        request_id: id,
                        responder: response_sender,
                    });

                self.rithmic_sender
                    .send(Message::Binary(logout_buf.into()))
                    .await
                    .unwrap();
            }
            TickerPlantCommand::SendHeartbeat => {
                let (heartbeat_buf, _id) =
                    self.rithmic_sender_api.request_heartbeat();

                let _ = self
                    .rithmic_sender
                    .send(Message::Binary(heartbeat_buf.into()))
                    .await;
            }
            TickerPlantCommand::UpdateHeartbeat { seconds } => {
                self.interval =
                    get_heartbeat_interval(Some(seconds));
            }
            TickerPlantCommand::Subscribe {
                symbol,
                exchange,
                fields,
                request_type,
                response_sender,
            } => {
                let (sub_buf, id) = self
                    .rithmic_sender_api
                    .request_market_data_update(
                        &symbol,
                        &exchange,
                        fields,
                        request_type,
                    );

                self.request_handler
                    .register_request(RithmicRequest {
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
                let (sub_buf, id) = self
                    .rithmic_sender_api
                    .request_depth_by_order_update(
                        &symbol,
                        &exchange,
                        request_type,
                    );

                self.request_handler
                    .register_request(RithmicRequest {
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
                    .request_depth_by_order_snapshot(
                        &symbol, &exchange,
                    );

                self.request_handler
                    .register_request(RithmicRequest {
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
                let (search_buf, id) = self
                    .rithmic_sender_api
                    .request_search_symbols(
                        &search_text,
                        exchange.as_deref(),
                        product_code.as_deref(),
                        instrument_type,
                        pattern,
                    );

                self.request_handler
                    .register_request(RithmicRequest {
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
                let (list_buf, id) = self
                    .rithmic_sender_api
                    .request_list_exchanges(&user);

                self.request_handler
                    .register_request(RithmicRequest {
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

                self.request_handler
                    .register_request(RithmicRequest {
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

                self.request_handler
                    .register_request(RithmicRequest {
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
                    .request_give_tick_size_type_table(
                        &tick_size_type,
                    );

                self.request_handler
                    .register_request(RithmicRequest {
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
                    .request_product_codes(
                        exchange.as_deref(),
                        give_toi_products_only,
                    );

                self.request_handler
                    .register_request(RithmicRequest {
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
                    .request_get_volume_at_price(
                        &symbol, &exchange,
                    );

                self.request_handler
                    .register_request(RithmicRequest {
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
                    .request_auxilliary_reference_data(
                        &symbol, &exchange,
                    );

                self.request_handler
                    .register_request(RithmicRequest {
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
                    .request_reference_data(
                        &symbol, &exchange,
                    );

                self.request_handler
                    .register_request(RithmicRequest {
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
                let (buf, id) = self
                    .rithmic_sender_api
                    .request_front_month_contract(
                        &symbol,
                        &exchange,
                        need_updates,
                    );

                self.request_handler
                    .register_request(RithmicRequest {
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
                    .request_rithmic_system_gateway_info(
                        system_name.as_deref(),
                    );

                self.request_handler
                    .register_request(RithmicRequest {
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

pub struct RithmicTickerPlantHandle {
    sender: mpsc::Sender<TickerPlantCommand>,
    subscription_sender: broadcast::Sender<RithmicResponse>,

    /// Receiver for subscription updates
    pub subscription_receiver: broadcast::Receiver<RithmicResponse>,
}

impl RithmicTickerPlantHandle {
    /// List available Rithmic system infrastructure information.
    ///
    /// Returns information about the connected Rithmic system, including
    /// system name, gateway info, and available services.
    pub async fn list_system_info(
        &self,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

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

    /// Log in to the Rithmic ticker plant
    ///
    /// This must be called before subscribing to any market data
    ///
    /// # Returns
    /// The login response or an error message
    pub async fn login(
        &self,
    ) -> Result<RithmicResponse, String> {
        info!("ticker_plant: logging in");

        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

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
            let _ = self
                .sender
                .send(TickerPlantCommand::SetLogin)
                .await;

            if let RithmicMessage::ResponseLogin(resp) =
                &response.message
            {
                if let Some(hb) = resp.heartbeat_interval {
                    let secs =
                        hb.max(HEARTBEAT_SECS as f64) as u64;
                    self.update_heartbeat(secs).await;
                }

                if let Some(session_id) = &resp.unique_user_id {
                    info!(
                        "ticker_plant: session id: {}",
                        session_id
                    );
                }
            }

            info!("ticker_plant: logged in");

            Ok(response)
        }
    }

    /// Disconnect from the Rithmic ticker plant
    ///
    /// # Returns
    /// The logout response or an error message
    pub async fn disconnect(
        &self,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::Logout {
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;
        let mut r = rx
            .await
            .map_err(|_| "Connection closed".to_string())??;
        let _ = self.sender.send(TickerPlantCommand::Close).await;
        let response = r.remove(0);

        self.subscription_sender
            .send(response.clone())
            .unwrap();

        Ok(response)
    }

    /// Subscribe to market data for a specific symbol
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    ///
    /// # Returns
    /// The subscription response or an error message
    pub async fn subscribe(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

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

    /// Subscribe to order book depth-by-order updates for a
    /// specific symbol
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    ///
    /// # Returns
    /// The subscription response or an error message
    pub async fn subscribe_order_book(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::SubscribeOrderBook {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            request_type:
                request_depth_by_order_updates::Request::Subscribe,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        Ok(rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0))
    }

    /// Unsubscribe from market data for a specific symbol
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    ///
    /// # Returns
    /// The unsubscription response or an error message
    pub async fn unsubscribe(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

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

    /// Unsubscribe from order book depth-by-order updates for a
    /// specific symbol
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    ///
    /// # Returns
    /// The unsubscription response or an error message
    pub async fn unsubscribe_order_book(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::SubscribeOrderBook {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            request_type:
                request_depth_by_order_updates::Request::Unsubscribe,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        Ok(rx
            .await
            .map_err(|_| "Connection closed".to_string())??
            .remove(0))
    }

    /// Request a snapshot of the order book depth-by-order for a
    /// specific symbol
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    ///
    /// # Returns
    /// A vector of responses containing the order book snapshot data
    /// or an error message
    pub async fn request_depth_by_order_snapshot(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command =
            TickerPlantCommand::RequestDepthByOrderSnapshot {
                symbol: symbol.to_string(),
                exchange: exchange.to_string(),
                response_sender: tx,
            };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    async fn update_heartbeat(&self, seconds: u64) {
        let command =
            TickerPlantCommand::UpdateHeartbeat { seconds };

        let _ = self.sender.send(command).await;
    }

    /// Search for symbols based on search criteria
    ///
    /// # Arguments
    /// * `search_text` - The text to search for in symbols
    /// * `exchange` - Optional exchange filter
    /// * `product_code` - Optional product code filter
    /// * `instrument_type` - Optional instrument type filter
    /// * `pattern` - Optional search pattern mode
    ///
    /// # Returns
    /// A vector of responses containing matching symbols or an
    /// error message
    pub async fn search_symbols(
        &self,
        search_text: &str,
        exchange: Option<&str>,
        product_code: Option<&str>,
        instrument_type: Option<
            request_search_symbols::InstrumentType,
        >,
        pattern: Option<request_search_symbols::Pattern>,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

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

    /// List exchanges available to the specified user
    ///
    /// # Arguments
    /// * `user` - The username to query exchange permissions for
    ///
    /// # Returns
    /// A vector of responses containing exchange information or an
    /// error message
    pub async fn list_exchanges(
        &self,
        user: &str,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::ListExchanges {
            user: user.to_string(),
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Get instruments by underlying symbol
    ///
    /// # Arguments
    /// * `underlying_symbol` - The underlying symbol (e.g., "ES")
    /// * `exchange` - The exchange code (e.g., "CME")
    /// * `expiration_date` - Optional expiration date filter
    ///
    /// # Returns
    /// A vector of responses containing instrument information or
    /// an error message
    pub async fn get_instrument_by_underlying(
        &self,
        underlying_symbol: &str,
        exchange: &str,
        expiration_date: Option<&str>,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command =
            TickerPlantCommand::GetInstrumentByUnderlying {
                underlying_symbol: underlying_symbol.to_string(),
                exchange: exchange.to_string(),
                expiration_date: expiration_date
                    .map(|d| d.to_string()),
                response_sender: tx,
            };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Subscribe to market data for all instruments of an
    /// underlying
    ///
    /// # Arguments
    /// * `underlying_symbol` - The underlying symbol (e.g., "ES")
    /// * `exchange` - The exchange code (e.g., "CME")
    /// * `expiration_date` - Optional expiration date filter
    /// * `fields` - Market data fields to subscribe to
    /// * `request_type` - Subscribe or Unsubscribe
    ///
    /// # Returns
    /// The subscription response or an error message
    pub async fn subscribe_by_underlying(
        &self,
        underlying_symbol: &str,
        exchange: &str,
        expiration_date: Option<&str>,
        fields: Vec<
            request_market_data_update_by_underlying::UpdateBits,
        >,
        request_type:
            request_market_data_update_by_underlying::Request,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::SubscribeByUnderlying {
            underlying_symbol: underlying_symbol.to_string(),
            exchange: exchange.to_string(),
            expiration_date: expiration_date
                .map(|d| d.to_string()),
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

    /// Get tick size type table
    ///
    /// # Arguments
    /// * `tick_size_type` - The tick size type identifier
    ///
    /// # Returns
    /// The tick size table response or an error message
    pub async fn get_tick_size_type_table(
        &self,
        tick_size_type: &str,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::GetTickSizeTypeTable {
            tick_size_type: tick_size_type.to_string(),
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Get product codes
    ///
    /// # Arguments
    /// * `exchange` - Optional exchange filter
    /// * `give_toi_products_only` - If true, only return Time of
    ///   Interest products
    ///
    /// # Returns
    /// A vector of responses containing product codes or an error
    /// message
    pub async fn get_product_codes(
        &self,
        exchange: Option<&str>,
        give_toi_products_only: Option<bool>,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::GetProductCodes {
            exchange: exchange.map(|e| e.to_string()),
            give_toi_products_only,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Get volume at price data
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    ///
    /// # Returns
    /// The volume at price response or an error message
    pub async fn get_volume_at_price(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = TickerPlantCommand::GetVolumeAtPrice {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| "Connection closed".to_string())?
    }

    /// Get auxiliary reference data for a symbol
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    ///
    /// # Returns
    /// The auxiliary reference data response or an error message
    pub async fn get_auxilliary_reference_data(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command =
            TickerPlantCommand::GetAuxilliaryReferenceData {
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

    /// Get reference data for a symbol
    ///
    /// Returns detailed information about a trading instrument
    /// including tick size, point value, trading hours, and other
    /// specifications.
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    ///
    /// # Returns
    /// The reference data response or an error message
    pub async fn get_reference_data(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

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

    /// Get front month contract
    ///
    /// Returns the current front month contract for a given
    /// product.
    ///
    /// # Arguments
    /// * `symbol` - The product symbol (e.g., "ES")
    /// * `exchange` - The exchange code (e.g., "CME")
    /// * `need_updates` - Whether to receive updates when front
    ///   month changes
    ///
    /// # Returns
    /// The front month contract response or an error message
    pub async fn get_front_month_contract(
        &self,
        symbol: &str,
        exchange: &str,
        need_updates: bool,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

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

    /// Get Rithmic system gateway info
    ///
    /// # Arguments
    /// * `system_name` - Optional system name to get info for
    ///
    /// # Returns
    /// The gateway info response or an error message
    pub async fn get_system_gateway_info(
        &self,
        system_name: Option<&str>,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) =
            oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

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
            subscription_receiver: self
                .subscription_sender
                .subscribe(),
            subscription_sender: self.subscription_sender.clone(),
        }
    }
}
