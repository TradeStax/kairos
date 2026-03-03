//! Rithmic History Plant actor.
//!
//! Manages a single WebSocket connection to Rithmic's History Plant
//! infrastructure. Provides historical tick bar replay, time bar replay,
//! volume profile queries, and live bar update subscriptions through an
//! async command-driven actor pattern.

use async_trait::async_trait;
use log::{error, info, warn};

use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    net::TcpStream,
    sync::{broadcast, mpsc, oneshot},
    task::JoinHandle,
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
        request_login::SysInfraType, request_tick_bar_update, request_time_bar_replay::BarType,
        request_time_bar_update,
    },
    sender::RithmicSenderApi,
    ws::{
        HEARTBEAT_SECS, PING_TIMEOUT_SECS, PlantActor, connect_with_strategy,
        get_heartbeat_interval, get_ping_interval,
    },
};

/// Commands sent to the history plant actor via its mpsc channel.
pub(crate) enum HistoryPlantCommand {
    /// Gracefully close the WebSocket connection
    Close,
    /// Query available Rithmic system infrastructure
    ListSystemInfo {
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Authenticate with the history plant
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
    /// Load historical tick bars for a time range
    LoadTicks {
        end_time_sec: i32,
        exchange: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
        start_time_sec: i32,
        symbol: String,
        /// Shared counter updated as responses are buffered
        buffered_count: Option<Arc<AtomicUsize>>,
    },
    /// Load historical time bars (OHLCV) for a time range
    LoadTimeBars {
        bar_type: BarType,
        bar_type_period: i32,
        end_time_sec: i32,
        exchange: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
        start_time_sec: i32,
        symbol: String,
    },
    /// Load volume profile minute bars for a time range
    LoadVolumeProfileMinuteBars {
        symbol: String,
        exchange: String,
        bar_type_period: i32,
        start_time_sec: i32,
        end_time_sec: i32,
        user_max_count: Option<i32>,
        resume_bars: Option<bool>,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Resume a previously truncated bars request
    ResumeBars {
        request_key: String,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
        /// Shared counter updated as responses are buffered
        buffered_count: Option<Arc<AtomicUsize>>,
    },
    /// Subscribe to or unsubscribe from live time bar updates
    SubscribeTimeBarUpdates {
        symbol: String,
        exchange: String,
        bar_type: request_time_bar_update::BarType,
        bar_type_period: i32,
        request: request_time_bar_update::Request,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
    /// Subscribe to or unsubscribe from live tick bar updates
    SubscribeTickBarUpdates {
        symbol: String,
        exchange: String,
        bar_type: request_tick_bar_update::BarType,
        bar_sub_type: request_tick_bar_update::BarSubType,
        bar_type_specifier: String,
        request: request_tick_bar_update::Request,
        response_sender: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
    },
}

/// Owns a history plant WebSocket actor task and its communication channels.
///
/// The background task runs until the WebSocket closes or a `Close`
/// command is sent. Use [`get_handle`](Self::get_handle) to obtain a
/// cloneable handle for sending commands.
pub struct RithmicHistoryPlant {
    /// Background task running the plant actor loop
    pub connection_handle: JoinHandle<()>,
    /// Command channel sender
    sender: mpsc::Sender<HistoryPlantCommand>,
    /// Broadcast sender for subscription updates
    subscription_sender: broadcast::Sender<RithmicResponse>,
}

impl RithmicHistoryPlant {
    /// Connects to Rithmic's History Plant and spawns the actor task.
    ///
    /// The plant is not yet authenticated -- call
    /// [`RithmicHistoryPlantHandle::login`] on the returned handle
    /// before issuing data requests.
    pub async fn connect(
        config: &RithmicConnectionConfig,
        strategy: ConnectStrategy,
    ) -> Result<RithmicHistoryPlant, Box<dyn std::error::Error>> {
        let (req_tx, req_rx) = mpsc::channel::<HistoryPlantCommand>(32);
        let (sub_tx, _sub_rx) = broadcast::channel::<RithmicResponse>(20_000);

        let mut history_plant = HistoryPlant::new(req_rx, sub_tx.clone(), config, strategy).await?;

        let connection_handle = tokio::spawn(async move {
            history_plant.run().await;
        });

        Ok(RithmicHistoryPlant {
            connection_handle,
            sender: req_tx,
            subscription_sender: sub_tx,
        })
    }
}

impl Drop for RithmicHistoryPlant {
    fn drop(&mut self) {
        self.connection_handle.abort();
    }
}

impl RithmicHistoryPlant {
    /// Returns a cloneable handle for sending commands to this plant
    pub fn get_handle(&self) -> RithmicHistoryPlantHandle {
        RithmicHistoryPlantHandle {
            sender: self.sender.clone(),
            subscription_receiver: self.subscription_sender.subscribe(),
            subscription_sender: self.subscription_sender.clone(),
        }
    }
}

/// Internal actor state for a history plant WebSocket connection.
#[derive(Debug)]
struct HistoryPlant {
    config: RithmicConnectionConfig,
    interval: Interval,
    logged_in: bool,
    ping_interval: Interval,
    ping_manager: PingManager,
    /// Buffer for `ResponseTickBarReplay` messages during active replay
    replay_buffer: Vec<RithmicResponse>,
    /// Shared counter for callers to observe buffering progress
    replay_counter: Option<Arc<AtomicUsize>>,
    /// Oneshot sender to return collected bars when replay completes
    replay_sender: Option<oneshot::Sender<Result<Vec<RithmicResponse>, String>>>,
    request_handler: RithmicRequestHandler,
    request_receiver: mpsc::Receiver<HistoryPlantCommand>,
    rithmic_reader: SplitStream<tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>>,
    rithmic_receiver_api: RithmicReceiverApi,
    rithmic_sender: SplitSink<
        tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>,
        tokio_tungstenite::tungstenite::Message,
    >,
    rithmic_sender_api: RithmicSenderApi,
    subscription_sender: broadcast::Sender<RithmicResponse>,
}

/// WebSocket send timeout (seconds). Prevents `handle_command` from
/// stalling indefinitely if the sink is blocked.
const WS_SEND_TIMEOUT_SECS: u64 = 10;

impl HistoryPlant {
    /// Send a WebSocket message with a timeout to prevent actor stall.
    async fn send_ws(&mut self, msg: Message) -> Result<(), String> {
        match tokio::time::timeout(
            std::time::Duration::from_secs(WS_SEND_TIMEOUT_SECS),
            self.rithmic_sender.send(msg),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(format!("history_plant: send failed: {}", e)),
            Err(_) => Err("history_plant: send timed out".to_owned()),
        }
    }

    pub async fn new(
        request_receiver: mpsc::Receiver<HistoryPlantCommand>,
        subscription_sender: broadcast::Sender<RithmicResponse>,
        config: &RithmicConnectionConfig,
        strategy: ConnectStrategy,
    ) -> Result<HistoryPlant, Box<dyn std::error::Error>> {
        let ws_stream = connect_with_strategy(&config.url, &config.beta_url, strategy).await?;

        let (rithmic_sender, rithmic_reader) = ws_stream.split();

        let rithmic_sender_api = RithmicSenderApi::new(config);
        let rithmic_receiver_api = RithmicReceiverApi {
            source: "history_plant".to_owned(),
        };

        let interval = get_heartbeat_interval(None);
        let ping_interval = get_ping_interval(None);
        let ping_manager = PingManager::new(PING_TIMEOUT_SECS);

        Ok(HistoryPlant {
            config: config.clone(),
            interval,
            ping_interval,
            logged_in: false,
            ping_manager,
            replay_buffer: Vec::new(),
            replay_counter: None,
            replay_sender: None,
            request_handler: RithmicRequestHandler::new(),
            request_receiver,
            rithmic_reader,
            rithmic_receiver_api,
            rithmic_sender,
            rithmic_sender_api,
            subscription_sender,
        })
    }
}

#[async_trait]
impl PlantActor for HistoryPlant {
    type Command = HistoryPlantCommand;

    async fn run(&mut self) {
        loop {
            tokio::select! {
              _ = self.interval.tick() => {
                if self.logged_in {
                    self.handle_command(
                        HistoryPlantCommand::SendHeartbeat,
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
                            "websocket_ping_timeout".to_owned(),
                        message:
                            Box::new(RithmicMessage::HeartbeatTimeout),
                        is_update: true,
                        has_more: false,
                        multi_response: false,
                        error: Some(
                            "WebSocket ping timeout \
                             - connection dead"
                                .to_owned(),
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
                match self.handle_rithmic_message(message).await {
                    Ok(true) => break,
                    Ok(false) => {},
                    Err(()) => {
                        error!("history_plant: message handler error, stopping");
                        break;
                    }
                }
              }
              else => { break; }
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
                info!("history_plant: Received close frame: {:?}", frame);
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
                        if matches!(*response.message, RithmicMessage::ResponseHeartbeat(_)) {
                            if let Some(error) = response.error {
                                let error_response = RithmicResponse {
                                    request_id: response.request_id,
                                    message: Box::new(RithmicMessage::HeartbeatTimeout),
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

                        // ResponseResumeBars (211) ack: when
                        // replay_sender is active (from ResumeBars
                        // command), silently consume the ack so it
                        // doesn't go to request_handler.
                        if matches!(*response.message, RithmicMessage::ResponseResumeBars(_))
                            && self.replay_sender.is_some()
                        {
                            if let Some(ref err) = response.error {
                                warn!(
                                    "history_plant: resume_bars \
                                     error: {}",
                                    err
                                );
                                if let Some(sender) = self.replay_sender.take() {
                                    let _ = sender.send(Err(err.clone()));
                                }
                                self.replay_buffer.clear();
                            }
                            // Success ack — just consume it; actual
                            // data follows as ResponseTickBarReplay
                        }
                        // Tick bar replay: ResponseTickBarReplay
                        // messages carry the actual bar data
                        // (is_update is always false for template
                        // 207). Buffer every data message
                        // (has_more=true) and send on completion
                        // (has_more=false).
                        else if matches!(
                            *response.message,
                            RithmicMessage::ResponseTickBarReplay(_)
                        ) && self.replay_sender.is_some()
                        {
                            if response.error.is_some() {
                                // Error — send immediately
                                self.replay_counter = None;
                                if let Some(sender) = self.replay_sender.take() {
                                    let _ = sender.send(Err(response.error.unwrap_or_default()));
                                }
                                self.replay_buffer.clear();
                            } else if response.has_more {
                                // Intermediate data row — buffer it
                                self.replay_buffer.push(response);
                                if let Some(ref counter) = self.replay_counter {
                                    counter.store(self.replay_buffer.len(), Ordering::Relaxed);
                                }
                            } else {
                                // Final message (has_more=false).
                                // It may also contain data, so
                                // include it in the buffer.
                                self.replay_buffer.push(response);
                                self.replay_counter = None;
                                let bars = std::mem::take(&mut self.replay_buffer);
                                if let Some(sender) = self.replay_sender.take() {
                                    info!(
                                        "history_plant: tick \
                                         replay complete, {} \
                                         bar(s) buffered",
                                        bars.len()
                                    );
                                    let _ = sender.send(Ok(bars));
                                }
                            }
                        } else if response.is_update {
                            match self.subscription_sender.send(response) {
                                Ok(_) => {}
                                Err(e) => {
                                    warn!(
                                        "history_plant: \
                                         no active subscribers: \
                                         {:?}",
                                        e
                                    );
                                }
                            }
                        } else {
                            self.request_handler.handle_response(response);
                        }
                    }
                    Err(err_response) => {
                        error!(
                            "history_plant: error response \
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
                error!("history_plant: connection closed");

                let error_response = RithmicResponse {
                    request_id: "".to_owned(),
                    message: Box::new(RithmicMessage::ConnectionError),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some("WebSocket connection closed".to_owned()),
                    source: self.rithmic_receiver_api.source.clone(),
                };
                let _ = self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::AlreadyClosed) => {
                error!("history_plant: connection already closed");

                let error_response = RithmicResponse {
                    request_id: "".to_owned(),
                    message: Box::new(RithmicMessage::ConnectionError),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some("WebSocket connection already closed".to_owned()),
                    source: self.rithmic_receiver_api.source.clone(),
                };
                let _ = self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::Io(ref io_err)) => {
                error!("history_plant: I/O error: {}", io_err);

                let error_response = RithmicResponse {
                    request_id: "".to_owned(),
                    message: Box::new(RithmicMessage::ConnectionError),
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
                    "history_plant: connection reset \
                     without closing handshake"
                );

                let error_response = RithmicResponse {
                    request_id: "".to_owned(),
                    message: Box::new(RithmicMessage::ConnectionError),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some(
                        "WebSocket connection reset \
                         without closing handshake"
                            .to_owned(),
                    ),
                    source: self.rithmic_receiver_api.source.clone(),
                };
                let _ = self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::Protocol(ProtocolError::SendAfterClosing)) => {
                error!(
                    "history_plant: attempted to send \
                     after closing"
                );

                let error_response = RithmicResponse {
                    request_id: "".to_owned(),
                    message: Box::new(RithmicMessage::ConnectionError),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some(
                        "WebSocket attempted to send \
                         after closing"
                            .to_owned(),
                    ),
                    source: self.rithmic_receiver_api.source.clone(),
                };
                let _ = self.subscription_sender.send(error_response);

                stop = true;
            }
            Err(Error::Protocol(ProtocolError::ReceivedAfterClosing)) => {
                error!("history_plant: received data after closing");

                let error_response = RithmicResponse {
                    request_id: "".to_owned(),
                    message: Box::new(RithmicMessage::ConnectionError),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some("WebSocket received data after closing".to_owned()),
                    source: self.rithmic_receiver_api.source.clone(),
                };
                let _ = self.subscription_sender.send(error_response);

                stop = true;
            }
            _ => {
                warn!("history_plant: Unhandled message {:?}", message);
            }
        }

        Ok(stop)
    }

    async fn handle_command(&mut self, command: HistoryPlantCommand) {
        match command {
            HistoryPlantCommand::Close => {
                let _ = self.send_ws(Message::Close(None)).await;
            }
            HistoryPlantCommand::ListSystemInfo { response_sender } => {
                let (list_system_info_buf, id) =
                    self.rithmic_sender_api.request_rithmic_system_info();

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                if let Err(e) = self
                    .send_ws(Message::Binary(list_system_info_buf.into()))
                    .await
                {
                    warn!("history_plant: send failed: {}", e);
                    return;
                }
            }
            HistoryPlantCommand::Login { response_sender } => {
                let (login_buf, id) = self.rithmic_sender_api.request_login(
                    &self.config.system_name,
                    SysInfraType::HistoryPlant,
                    &self.config.user,
                    &self.config.password,
                );

                info!("history_plant: sending login request {}", id);

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                if let Err(e) = self.send_ws(Message::Binary(login_buf.into())).await {
                    warn!("history_plant: send failed: {}", e);
                    return;
                }
            }
            HistoryPlantCommand::SetLogin => {
                self.logged_in = true;
            }
            HistoryPlantCommand::Logout { response_sender } => {
                let (logout_buf, id) = self.rithmic_sender_api.request_logout();

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                if let Err(e) = self.send_ws(Message::Binary(logout_buf.into())).await {
                    warn!("history_plant: send failed: {}", e);
                    return;
                }
            }
            HistoryPlantCommand::SendHeartbeat => {
                let (heartbeat_bf, _id) = self.rithmic_sender_api.request_heartbeat();

                let _ = self.send_ws(Message::Binary(heartbeat_bf.into())).await;
            }
            HistoryPlantCommand::UpdateHeartbeat { seconds } => {
                self.interval = get_heartbeat_interval(Some(seconds));
            }
            HistoryPlantCommand::LoadTicks {
                exchange,
                symbol,
                start_time_sec,
                end_time_sec,
                response_sender,
                buffered_count,
            } => {
                // Clear any stale replay state and store the
                // response sender directly.
                // ResponseTickBarReplay (207) messages are buffered
                // in handle_rithmic_message until the final message
                // (has_more=false), then sent via the oneshot.
                if self.replay_sender.is_some() {
                    warn!(
                        "history_plant: overwriting pending replay_sender \
                         (previous request will never receive a response)"
                    );
                }
                self.replay_buffer.clear();
                self.replay_counter = buffered_count;
                self.replay_sender = Some(response_sender);

                let (tick_bar_replay_buf, _id) = self.rithmic_sender_api.request_tick_bar_replay(
                    &symbol,
                    &exchange,
                    start_time_sec,
                    end_time_sec,
                );

                if let Err(e) = self
                    .send_ws(Message::Binary(tick_bar_replay_buf.into()))
                    .await
                {
                    warn!("history_plant: send failed: {}", e);
                    return;
                }
            }
            HistoryPlantCommand::LoadTimeBars {
                bar_type,
                bar_type_period,
                end_time_sec,
                exchange,
                response_sender,
                start_time_sec,
                symbol,
            } => {
                let (time_bar_replay_buf, id) = self.rithmic_sender_api.request_time_bar_replay(
                    &symbol,
                    &exchange,
                    bar_type,
                    bar_type_period,
                    start_time_sec,
                    end_time_sec,
                );

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                if let Err(e) = self
                    .send_ws(Message::Binary(time_bar_replay_buf.into()))
                    .await
                {
                    warn!("history_plant: send failed: {}", e);
                    return;
                }
            }
            HistoryPlantCommand::LoadVolumeProfileMinuteBars {
                symbol,
                exchange,
                bar_type_period,
                start_time_sec,
                end_time_sec,
                user_max_count,
                resume_bars,
                response_sender,
            } => {
                let (buf, id) = self.rithmic_sender_api.request_volume_profile_minute_bars(
                    &symbol,
                    &exchange,
                    bar_type_period,
                    start_time_sec,
                    end_time_sec,
                    user_max_count,
                    resume_bars,
                );

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                if let Err(e) = self.send_ws(Message::Binary(buf.into())).await {
                    warn!("history_plant: send failed: {}", e);
                    return;
                }
            }
            HistoryPlantCommand::ResumeBars {
                request_key,
                response_sender,
                buffered_count,
            } => {
                // Use replay buffer (same as LoadTicks) so that
                // the ResponseTickBarReplay (207) data messages
                // are collected correctly.
                if self.replay_sender.is_some() {
                    warn!(
                        "history_plant: overwriting pending replay_sender \
                         (previous request will never receive a response)"
                    );
                }
                self.replay_buffer.clear();
                self.replay_counter = buffered_count;
                self.replay_sender = Some(response_sender);

                let (buf, _id) = self.rithmic_sender_api.request_resume_bars(&request_key);

                if let Err(e) = self.send_ws(Message::Binary(buf.into())).await {
                    warn!("history_plant: send failed: {}", e);
                    return;
                }
            }
            HistoryPlantCommand::SubscribeTimeBarUpdates {
                symbol,
                exchange,
                bar_type,
                bar_type_period,
                request,
                response_sender,
            } => {
                let (buf, id) = self.rithmic_sender_api.request_time_bar_update(
                    &symbol,
                    &exchange,
                    bar_type,
                    bar_type_period,
                    request,
                );

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                if let Err(e) = self.send_ws(Message::Binary(buf.into())).await {
                    warn!("history_plant: send failed: {}", e);
                    return;
                }
            }
            HistoryPlantCommand::SubscribeTickBarUpdates {
                symbol,
                exchange,
                bar_type,
                bar_sub_type,
                bar_type_specifier,
                request,
                response_sender,
            } => {
                let (buf, id) = self.rithmic_sender_api.request_tick_bar_update(
                    &symbol,
                    &exchange,
                    bar_type,
                    bar_sub_type,
                    &bar_type_specifier,
                    request,
                );

                self.request_handler.register_request(RithmicRequest {
                    request_id: id,
                    responder: response_sender,
                });

                if let Err(e) = self.send_ws(Message::Binary(buf.into())).await {
                    warn!("history_plant: send failed: {}", e);
                    return;
                }
            }
        }
    }
}

/// Cloneable handle for interacting with a history plant actor.
///
/// Provides async methods for login, data loading, and bar
/// subscriptions. Each method sends a command to the actor task
/// and awaits the response via a oneshot channel.
pub struct RithmicHistoryPlantHandle {
    /// Command channel to the actor task
    sender: mpsc::Sender<HistoryPlantCommand>,
    /// Used to create new broadcast receivers on clone
    subscription_sender: broadcast::Sender<RithmicResponse>,
    /// Broadcast receiver for subscription updates
    pub subscription_receiver: broadcast::Receiver<RithmicResponse>,
}

impl RithmicHistoryPlantHandle {
    /// Sends a close command to gracefully shut down the plant actor.
    pub async fn close(&self) {
        let _ = self.sender.send(HistoryPlantCommand::Close).await;
    }

    /// Lists available Rithmic system infrastructure information
    pub async fn list_system_info(&self) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = HistoryPlantCommand::ListSystemInfo {
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        super::take_first(rx.await.map_err(|_| super::CONN_CLOSED_ERR.to_owned())??)
    }

    /// Logs in to the Rithmic History Plant.
    ///
    /// Must be called before any data requests. Configures the
    /// heartbeat interval from the server's login response.
    pub async fn login(&self) -> Result<RithmicResponse, String> {
        info!("history_plant: logging in ");

        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = HistoryPlantCommand::Login {
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;
        let response =
            super::take_first(rx.await.map_err(|_| super::CONN_CLOSED_ERR.to_owned())??)?;

        if let Some(err) = response.error {
            error!("history_plant: login failed {:?}", err);
            Err(err)
        } else {
            let _ = self.sender.send(HistoryPlantCommand::SetLogin).await;

            if let RithmicMessage::ResponseLogin(resp) = &*response.message {
                if let Some(hb) = resp.heartbeat_interval {
                    let secs = hb.max(HEARTBEAT_SECS as f64) as u64;
                    self.update_heartbeat(secs).await;
                }

                if let Some(session_id) = &resp.unique_user_id {
                    info!("history_plant: session id: {}", session_id);
                }
            }

            info!("history_plant: logged in");

            Ok(response)
        }
    }

    /// Sends a command to update the heartbeat interval
    async fn update_heartbeat(&self, seconds: u64) {
        let command = HistoryPlantCommand::UpdateHeartbeat { seconds };

        let _ = self.sender.send(command).await;
    }

    /// Disconnects from the Rithmic History Plant (logout + close)
    pub async fn disconnect(&self) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = HistoryPlantCommand::Logout {
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;
        let response =
            super::take_first(rx.await.map_err(|_| super::CONN_CLOSED_ERR.to_owned())??)?;
        let _ = self.sender.send(HistoryPlantCommand::Close).await;

        Ok(response)
    }

    /// Loads historical tick data for a symbol and time range.
    ///
    /// Responses are buffered internally until the server signals
    /// completion (`has_more = false`), then returned as a batch.
    pub async fn load_ticks(
        &self,
        symbol: String,
        exchange: String,
        start_time_sec: i32,
        end_time_sec: i32,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = HistoryPlantCommand::LoadTicks {
            exchange,
            symbol,
            start_time_sec,
            end_time_sec,
            response_sender: tx,
            buffered_count: None,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| super::CONN_CLOSED_ERR.to_owned())?
    }

    /// Loads historical tick data with real-time buffering progress.
    ///
    /// Like [`load_ticks`] but polls a shared atomic counter to
    /// report buffering progress via `on_progress(buffered_count)`
    /// while the plant actor buffers WebSocket responses.
    pub async fn load_ticks_with_buffering_progress(
        &self,
        symbol: String,
        exchange: String,
        start_time_sec: i32,
        end_time_sec: i32,
        on_progress: &(dyn Fn(usize) + Send + Sync),
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();
        let counter = Arc::new(AtomicUsize::new(0));

        let command = HistoryPlantCommand::LoadTicks {
            exchange,
            symbol,
            start_time_sec,
            end_time_sec,
            response_sender: tx,
            buffered_count: Some(counter.clone()),
        };

        let _ = self.sender.send(command).await;

        // Poll the counter while awaiting the oneshot result.
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
        tokio::pin!(rx);
        loop {
            tokio::select! {
                biased;
                res = &mut rx => {
                    break res
                        .map_err(|_| super::CONN_CLOSED_ERR.to_owned())?;
                }
                _ = interval.tick() => {
                    let count = counter.load(Ordering::Relaxed);
                    if count > 0 {
                        on_progress(count);
                    }
                }
            }
        }
    }

    /// Loads historical time bar data (OHLCV) for a symbol and time range
    pub async fn load_time_bars(
        &self,
        symbol: String,
        exchange: String,
        bar_type: BarType,
        bar_type_period: i32,
        start_time_sec: i32,
        end_time_sec: i32,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = HistoryPlantCommand::LoadTimeBars {
            bar_type,
            bar_type_period,
            end_time_sec,
            exchange,
            response_sender: tx,
            start_time_sec,
            symbol,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| super::CONN_CLOSED_ERR.to_owned())?
    }

    /// Loads volume profile minute bars for a symbol and time range
    #[allow(clippy::too_many_arguments)]
    pub async fn load_volume_profile_minute_bars(
        &self,
        symbol: String,
        exchange: String,
        bar_type_period: i32,
        start_time_sec: i32,
        end_time_sec: i32,
        user_max_count: Option<i32>,
        resume_bars: Option<bool>,
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = HistoryPlantCommand::LoadVolumeProfileMinuteBars {
            symbol,
            exchange,
            bar_type_period,
            start_time_sec,
            end_time_sec,
            user_max_count,
            resume_bars,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| super::CONN_CLOSED_ERR.to_owned())?
    }

    /// Resumes a previously truncated bars request using its
    /// `request_key`
    pub async fn resume_bars(&self, request_key: String) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = HistoryPlantCommand::ResumeBars {
            request_key,
            response_sender: tx,
            buffered_count: None,
        };

        let _ = self.sender.send(command).await;

        rx.await.map_err(|_| super::CONN_CLOSED_ERR.to_owned())?
    }

    /// Resumes a previously truncated bars request with buffering
    /// progress reporting.
    pub async fn resume_bars_with_buffering_progress(
        &self,
        request_key: String,
        on_progress: &(dyn Fn(usize) + Send + Sync),
    ) -> Result<Vec<RithmicResponse>, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();
        let counter = Arc::new(AtomicUsize::new(0));

        let command = HistoryPlantCommand::ResumeBars {
            request_key,
            response_sender: tx,
            buffered_count: Some(counter.clone()),
        };

        let _ = self.sender.send(command).await;

        let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
        tokio::pin!(rx);
        loop {
            tokio::select! {
                biased;
                res = &mut rx => {
                    break res
                        .map_err(|_| super::CONN_CLOSED_ERR.to_owned())?;
                }
                _ = interval.tick() => {
                    let count = counter.load(Ordering::Relaxed);
                    if count > 0 {
                        on_progress(count);
                    }
                }
            }
        }
    }

    /// Subscribes to or unsubscribes from live time bar updates
    pub async fn subscribe_time_bar_updates(
        &self,
        symbol: &str,
        exchange: &str,
        bar_type: request_time_bar_update::BarType,
        bar_type_period: i32,
        request: request_time_bar_update::Request,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = HistoryPlantCommand::SubscribeTimeBarUpdates {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            bar_type,
            bar_type_period,
            request,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        super::take_first(rx.await.map_err(|_| super::CONN_CLOSED_ERR.to_owned())??)
    }

    /// Subscribes to or unsubscribes from live tick bar updates
    pub async fn subscribe_tick_bar_updates(
        &self,
        symbol: &str,
        exchange: &str,
        bar_type: request_tick_bar_update::BarType,
        bar_sub_type: request_tick_bar_update::BarSubType,
        bar_type_specifier: &str,
        request: request_tick_bar_update::Request,
    ) -> Result<RithmicResponse, String> {
        let (tx, rx) = oneshot::channel::<Result<Vec<RithmicResponse>, String>>();

        let command = HistoryPlantCommand::SubscribeTickBarUpdates {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            bar_type,
            bar_sub_type,
            bar_type_specifier: bar_type_specifier.to_string(),
            request,
            response_sender: tx,
        };

        let _ = self.sender.send(command).await;

        super::take_first(rx.await.map_err(|_| super::CONN_CLOSED_ERR.to_owned())??)
    }
}

impl Clone for RithmicHistoryPlantHandle {
    fn clone(&self) -> Self {
        RithmicHistoryPlantHandle {
            sender: self.sender.clone(),
            subscription_receiver: self.subscription_sender.subscribe(),
            subscription_sender: self.subscription_sender.clone(),
        }
    }
}
