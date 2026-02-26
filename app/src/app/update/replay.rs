use iced::Task;

use crate::components::display::toast::Toast;
use crate::modals::replay;
use crate::screen::dashboard;

use super::super::{Kairos, Message};

impl Kairos {
    pub(crate) fn handle_replay_message(&mut self, msg: replay::Message) -> Task<Message> {
        match msg {
            replay::Message::Close => {
                self.ui.sidebar.set_menu(None);
                return Task::none();
            }
            replay::Message::LoadData => {
                return self.replay_load_data();
            }
            replay::Message::Play => {
                return self.replay_engine_action(|engine| Box::pin(engine.play()));
            }
            replay::Message::Pause => {
                return self.replay_engine_action(|engine| Box::pin(engine.pause()));
            }
            replay::Message::EndReplay => {
                // Stop replay, restore chart data, hide controller
                self.modals.replay_manager.data_loaded = false;
                self.modals.replay_manager.progress = 0.0;
                self.modals.replay_manager.position = 0;
                self.modals.replay_manager.playback_status = data::PlaybackStatus::Stopped;
                self.modals.replay_manager.controller_visible = false;
                self.modals.replay_manager.volume_buckets.clear();

                self.exit_replay_on_all_panes();

                return self.replay_engine_action(|engine| Box::pin(engine.stop()));
            }
            replay::Message::CloseController => {
                // Hide controller but keep replay playing
                self.modals.replay_manager.controller_visible = false;
            }
            replay::Message::OpenController => {
                self.modals.replay_manager.controller_visible = true;
            }
            replay::Message::SetSpeed(speed) => {
                self.modals.replay_manager.speed = speed;
                return self.replay_engine_action(move |engine| Box::pin(engine.set_speed(speed)));
            }
            replay::Message::Seek(progress) => {
                return self.replay_seek(progress);
            }
            replay::Message::JumpForward => {
                return self.replay_engine_action(|engine| Box::pin(engine.jump(30_000)));
            }
            replay::Message::JumpBackward => {
                return self.replay_engine_action(|engine| Box::pin(engine.jump(-30_000)));
            }
            other => {
                // UI-only messages (SelectStream, SelectDate, etc.)
                self.modals.replay_manager.update(other);
            }
        }
        Task::none()
    }

    pub(crate) fn handle_replay_event(
        &mut self,
        event: crate::services::ReplayEvent,
    ) -> Task<Message> {
        match event {
            crate::services::ReplayEvent::MarketData {
                timestamp: _,
                trades,
                depth: _,
            } => {
                if trades.is_empty() {
                    return Task::none();
                }

                let ticker_info = self
                    .modals
                    .replay_manager
                    .selected_stream
                    .as_ref()
                    .map(|s| s.ticker_info);

                let Some(info) = ticker_info else {
                    return Task::none();
                };

                // Send ALL trades directly to pane Content (not ChartState)
                return Task::done(Message::Dashboard {
                    layout_id: None,
                    event: Box::new(dashboard::Message::ReplayTrades(info, trades)),
                });
            }
            crate::services::ReplayEvent::PositionUpdate {
                timestamp,
                progress,
            } => {
                self.modals.replay_manager.position = timestamp;
                self.modals.replay_manager.progress = progress;
            }
            crate::services::ReplayEvent::StatusChanged(status) => {
                self.modals.replay_manager.playback_status = status;
            }
            crate::services::ReplayEvent::DataLoaded {
                ticker: _,
                trade_count,
                depth_count,
                time_range,
            } => {
                self.modals.replay_manager.trade_count = trade_count;
                self.modals.replay_manager.depth_count = depth_count;
                self.modals.replay_manager.time_range = Some(time_range);
                self.modals.replay_manager.data_loaded = true;
                self.modals.replay_manager.loading_progress = None;
                self.modals.replay_manager.error = None;

                // Show the floating controller
                self.modals.replay_manager.controller_visible = true;

                // Enter replay mode on matching panes
                self.enter_replay_on_matching_panes();

                // Spawn task to compute volume histogram
                return self.compute_volume_histogram();
            }
            crate::services::ReplayEvent::LoadingProgress { progress, message } => {
                self.modals.replay_manager.loading_progress = Some((progress, message));
            }
            crate::services::ReplayEvent::Error(msg) => {
                self.modals.replay_manager.error = Some(msg.clone());
                self.modals.replay_manager.loading_progress = None;
                self.ui
                    .push_notification(Toast::error(format!("Replay: {}", msg)));
            }
            crate::services::ReplayEvent::PlaybackComplete => {
                self.modals.replay_manager.playback_status = data::PlaybackStatus::Stopped;
                self.modals.replay_manager.progress = 1.0;
            }
            crate::services::ReplayEvent::SeekCompleted {
                timestamp,
                progress,
            } => {
                self.modals.replay_manager.position = timestamp;
                self.modals.replay_manager.progress = progress;
            }
            crate::services::ReplayEvent::ChartRebuild { trades } => {
                let ticker_info = self
                    .modals
                    .replay_manager
                    .selected_stream
                    .as_ref()
                    .map(|s| s.ticker_info);

                if let Some(info) = ticker_info {
                    return Task::done(Message::Dashboard {
                        layout_id: None,
                        event: Box::new(dashboard::Message::ReplayRebuild(info, trades)),
                    });
                }
            }
            _ => {}
        }
        Task::none()
    }

    /// Run an async action on the replay engine via Task::perform.
    fn replay_engine_action<F>(&self, action: F) -> Task<Message>
    where
        F: for<'a> FnOnce(
                &'a mut crate::services::ReplayEngine,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>,
            > + Send
            + 'static,
    {
        let Some(engine) = self.services.replay_engine.clone() else {
            return Task::none();
        };

        Task::perform(
            async move {
                let mut guard = engine.lock().await;
                action(&mut guard).await
            },
            |result| match result {
                Ok(()) => Message::Tick(std::time::Instant::now()),
                Err(e) => Message::ReplayEvent(crate::services::ReplayEvent::Error(e)),
            },
        )
    }

    fn replay_load_data(&mut self) -> Task<Message> {
        let Some(ref stream) = self.modals.replay_manager.selected_stream else {
            return Task::none();
        };

        let Some(engine) = self.services.replay_engine.clone() else {
            self.modals.replay_manager.error = Some("Replay engine not available".to_string());
            return Task::none();
        };

        let ticker_info = stream.ticker_info;
        let date_range = stream.date_range;
        let replay_sender = super::super::core::globals::get_replay_sender();

        // Compute the user-specified start timestamp from date + time fields
        let start_timestamp = self.compute_replay_start_timestamp();

        // Set initial loading state
        self.modals.replay_manager.loading_progress = Some((0.0, "Starting load...".to_string()));
        self.modals.replay_manager.error = None;

        Task::perform(
            async move {
                let mut guard = engine.lock().await;

                // Take the event_rx and bridge to the global channel sender
                if let Some(rx) = guard.take_event_rx() {
                    tokio::spawn(async move {
                        let mut rx = rx;
                        while let Some(event) = rx.recv().await {
                            let _ = replay_sender.send(event);
                        }
                    });
                }

                // Load data, seek to user-specified start, then play
                guard.load_data(ticker_info, date_range).await?;

                // Seek to user-specified start time if provided
                if let Some(ts) = start_timestamp {
                    guard.seek(ts).await?;
                }

                guard.play().await?;
                Ok::<(), String>(())
            },
            |result| match result {
                Ok(()) => Message::ReplayEvent(crate::services::ReplayEvent::PlaybackStarted),
                Err(e) => Message::ReplayEvent(crate::services::ReplayEvent::Error(e)),
            },
        )
    }

    /// Parse the user's start_date + start_time into a millisecond timestamp.
    /// The input is interpreted in the user's configured timezone.
    /// Returns None if the fields are empty or invalid.
    fn compute_replay_start_timestamp(&self) -> Option<u64> {
        let date_str = &self.modals.replay_manager.start_date;
        let time_str = &self.modals.replay_manager.start_time;

        if date_str.is_empty() {
            return None;
        }

        let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
        let time = if time_str.is_empty() {
            chrono::NaiveTime::from_hms_opt(0, 0, 0)?
        } else {
            chrono::NaiveTime::parse_from_str(time_str, "%H:%M:%S").ok()?
        };

        let dt = chrono::NaiveDateTime::new(date, time);
        let utc_millis = self.ui.timezone.naive_to_utc_millis(dt);
        Some(utc_millis as u64)
    }

    /// Compute volume histogram from loaded data and deliver to UI.
    fn compute_volume_histogram(&self) -> Task<Message> {
        let Some(engine) = self.services.replay_engine.clone() else {
            return Task::none();
        };

        Task::perform(
            async move {
                let guard = engine.lock().await;
                guard.compute_volume_histogram(200).await
            },
            |buckets| Message::Replay(replay::Message::VolumeHistogramReady(buckets)),
        )
    }

    /// Enter replay mode on all panes matching the selected stream's ticker.
    fn enter_replay_on_matching_panes(&mut self) {
        let Some(ref stream) = self.modals.replay_manager.selected_stream else {
            return;
        };
        let ticker = stream.ticker_info.ticker;

        let Some(dashboard) = self.active_dashboard_mut() else {
            return;
        };
        for (_, state) in dashboard.panes.iter_mut() {
            if let Some(ti) = state.ticker_info
                && ti.ticker == ticker
            {
                state.enter_replay_mode();
            }
        }
        for (_, (popout_panes, _)) in dashboard.popout.iter_mut() {
            for (_, state) in popout_panes.iter_mut() {
                if let Some(ti) = state.ticker_info
                    && ti.ticker == ticker
                {
                    state.enter_replay_mode();
                }
            }
        }
    }

    /// Exit replay mode on all panes (restore original data).
    fn exit_replay_on_all_panes(&mut self) {
        let Some(dashboard) = self.active_dashboard_mut() else {
            return;
        };
        for (_, state) in dashboard.panes.iter_mut() {
            if state.is_replaying() {
                state.exit_replay_mode();
            }
        }
        for (_, (popout_panes, _)) in dashboard.popout.iter_mut() {
            for (_, state) in popout_panes.iter_mut() {
                if state.is_replaying() {
                    state.exit_replay_mode();
                }
            }
        }
    }

    fn replay_seek(&mut self, progress: f32) -> Task<Message> {
        self.modals.replay_manager.progress = progress;

        let Some(ref range) = self.modals.replay_manager.time_range else {
            return Task::none();
        };

        let start = range.start.to_millis();
        let end = range.end.to_millis();
        let timestamp = start + ((end - start) as f32 * progress) as u64;

        self.replay_engine_action(move |engine| Box::pin(engine.seek(timestamp)))
    }
}
