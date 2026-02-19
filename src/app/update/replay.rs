use iced::Task;

use crate::component::display::toast::Toast;
use crate::modal::replay_manager;
use crate::screen::dashboard;

use super::super::{Flowsurface, Message};

impl Flowsurface {
    pub(crate) fn handle_replay_message(
        &mut self,
        msg: replay_manager::Message,
    ) -> Task<Message> {
        match msg {
            replay_manager::Message::LoadData => {
                return self.replay_load_data();
            }
            replay_manager::Message::Play => {
                return self.replay_engine_action(|engine| {
                    tokio::runtime::Handle::current().block_on(engine.play())
                });
            }
            replay_manager::Message::Pause => {
                return self.replay_engine_action(|engine| {
                    tokio::runtime::Handle::current().block_on(engine.pause())
                });
            }
            replay_manager::Message::Stop => {
                self.replay_manager.data_loaded = false;
                self.replay_manager.progress = 0.0;
                self.replay_manager.position = 0;
                self.replay_manager.playback_status =
                    data::state::replay_state::PlaybackStatus::Stopped;

                return self.replay_engine_action(|engine| {
                    tokio::runtime::Handle::current().block_on(engine.stop())
                });
            }
            replay_manager::Message::SetSpeed(speed) => {
                self.replay_manager.speed = speed;
                return self.replay_engine_action(move |engine| {
                    tokio::runtime::Handle::current()
                        .block_on(engine.set_speed(speed))
                });
            }
            replay_manager::Message::Seek(progress) => {
                return self.replay_seek(progress);
            }
            replay_manager::Message::JumpForward => {
                return self.replay_engine_action(|engine| {
                    tokio::runtime::Handle::current().block_on(engine.jump(30_000))
                });
            }
            replay_manager::Message::JumpBackward => {
                return self.replay_engine_action(|engine| {
                    tokio::runtime::Handle::current().block_on(engine.jump(-30_000))
                });
            }
            other => {
                // UI-only messages (SelectStream, SetStartDate, etc.)
                self.replay_manager.update(other);
            }
        }
        Task::none()
    }

    pub(crate) fn handle_replay_event(
        &mut self,
        event: data::services::ReplayEvent,
    ) -> Task<Message> {
        match event {
            data::services::ReplayEvent::MarketData {
                timestamp: _,
                trades,
                depth: _,
            } => {
                // Convert data::Trade to exchange events and route to dashboard
                // We batch all trades in a single message for efficiency
                if trades.is_empty() {
                    return Task::none();
                }

                let ticker_info = self
                    .replay_manager
                    .selected_stream
                    .as_ref()
                    .map(|s| s.ticker_info);

                let Some(info) = ticker_info else {
                    return Task::none();
                };

                let stream_kind = exchange::adapter::StreamKind::DepthAndTrades {
                    ticker_info: info,
                    depth_aggr: exchange::adapter::StreamTicksize::Client,
                    push_freq: exchange::PushFrequency::ServerDefault,
                };

                // Send last trade as exchange event to trigger chart update
                if let Some(trade) = trades.last() {
                    let exchange_trade = exchange::Trade {
                        time: trade.time.to_millis(),
                        price: trade.price.to_f32(),
                        qty: trade.quantity.0 as f32,
                        side: if trade.side == data::Side::Sell {
                            exchange::TradeSide::Sell
                        } else {
                            exchange::TradeSide::Buy
                        },
                    };

                    return Task::done(Message::Dashboard {
                        layout_id: None,
                        event: dashboard::Message::ExchangeEvent(
                            exchange::Event::TradeReceived(stream_kind, exchange_trade),
                        ),
                    });
                }
            }
            data::services::ReplayEvent::PositionUpdate {
                timestamp,
                progress,
            } => {
                self.replay_manager.position = timestamp;
                self.replay_manager.progress = progress;
            }
            data::services::ReplayEvent::StatusChanged(status) => {
                self.replay_manager.playback_status = status;
            }
            data::services::ReplayEvent::DataLoaded {
                ticker: _,
                trade_count,
                depth_count,
                time_range,
            } => {
                self.replay_manager.trade_count = trade_count;
                self.replay_manager.depth_count = depth_count;
                self.replay_manager.time_range = Some(time_range);
                self.replay_manager.data_loaded = true;
                self.replay_manager.loading_progress = None;
                self.replay_manager.error = None;
            }
            data::services::ReplayEvent::LoadingProgress { progress, message } => {
                self.replay_manager.loading_progress = Some((progress, message));
            }
            data::services::ReplayEvent::Error(msg) => {
                self.replay_manager.error = Some(msg.clone());
                self.replay_manager.loading_progress = None;
                self.notifications
                    .push(Toast::error(format!("Replay: {}", msg)));
            }
            data::services::ReplayEvent::PlaybackComplete => {
                self.replay_manager.playback_status =
                    data::state::replay_state::PlaybackStatus::Stopped;
                self.replay_manager.progress = 1.0;
            }
            _ => {}
        }
        Task::none()
    }

    /// Run a synchronous action on the replay engine via spawn_blocking.
    /// The closure receives a `&mut ReplayEngine` and should use
    /// `Handle::current().block_on(...)` for any async engine methods.
    fn replay_engine_action<F>(&self, action: F) -> Task<Message>
    where
        F: FnOnce(&mut data::services::ReplayEngine) -> Result<(), String>
            + Send
            + 'static,
    {
        let Some(engine) = self.replay_engine.clone() else {
            return Task::none();
        };

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut guard = engine.lock().unwrap_or_else(|e| e.into_inner());
                    action(&mut guard)
                })
                .await
                .map_err(|e| format!("Task join error: {}", e))?
            },
            |result| match result {
                Ok(()) => {
                    // Events come through the subscription, no need to emit here
                    Message::Tick(std::time::Instant::now())
                }
                Err(e) => Message::ReplayEvent(data::services::ReplayEvent::Error(e)),
            },
        )
    }

    fn replay_load_data(&mut self) -> Task<Message> {
        let Some(ref stream) = self.replay_manager.selected_stream else {
            return Task::none();
        };

        let Some(engine) = self.replay_engine.clone() else {
            self.replay_manager.error = Some("Replay engine not available".to_string());
            return Task::none();
        };

        let ticker_info = stream.ticker_info;
        let date_range = stream.date_range;
        let events_buf = super::super::get_replay_events().clone();

        // Set initial loading state
        self.replay_manager.loading_progress =
            Some((0.0, "Starting load...".to_string()));
        self.replay_manager.error = None;

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let handle = tokio::runtime::Handle::current();
                    let mut guard =
                        engine.lock().unwrap_or_else(|e| e.into_inner());

                    // Take the event_rx and bridge to global buffer
                    if let Some(rx) = guard.event_rx.take() {
                        let buf = events_buf.clone();
                        handle.spawn(async move {
                            let mut rx = rx;
                            while let Some(event) = rx.recv().await {
                                if let Ok(mut b) = buf.lock() {
                                    b.push(event);
                                }
                            }
                        });
                    }

                    // Load data and start playback (blocking on async)
                    handle.block_on(async {
                        guard.load_data(ticker_info, date_range).await?;
                        guard.play().await?;
                        Ok::<(), String>(())
                    })
                })
                .await
                .map_err(|e| format!("Task join error: {}", e))?
            },
            |result| match result {
                Ok(()) => Message::ReplayEvent(
                    data::services::ReplayEvent::PlaybackStarted,
                ),
                Err(e) => Message::ReplayEvent(
                    data::services::ReplayEvent::Error(e),
                ),
            },
        )
    }

    fn replay_seek(&mut self, progress: f32) -> Task<Message> {
        self.replay_manager.progress = progress;

        let Some(ref range) = self.replay_manager.time_range else {
            return Task::none();
        };

        let start = range.start.to_millis();
        let end = range.end.to_millis();
        let timestamp = start + ((end - start) as f32 * progress) as u64;

        self.replay_engine_action(move |engine| {
            tokio::runtime::Handle::current().block_on(engine.seek(timestamp))
        })
    }
}
