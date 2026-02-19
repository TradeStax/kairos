use super::{Dashboard, Event, Message, pane};
use crate::{component::display::toast::Toast, window};
use data::{ChartConfig, ChartData, ChartState, ContentKind, DateRange, LinkGroup, LoadingStatus};
use exchange::FuturesTickerInfo;
use iced::Task;

impl Dashboard {
    pub fn init_focused_pane(
        &mut self,
        main_window: window::Id,
        ticker_info: FuturesTickerInfo,
        content_kind: ContentKind,
    ) -> Task<Message> {
        log::info!(
            "DASHBOARD: init_focused_pane called with {:?} ContentKind::{:?}",
            ticker_info.ticker,
            content_kind
        );

        // Get the focused pane
        let Some((window, pane)) = self.focus else {
            log::warn!("No pane focused when trying to initialize");
            return Task::done(Message::Notification(Toast::warn(
                "No pane selected".to_string(),
            )));
        };

        // Get registered date range BEFORE borrowing pane_state mutably
        let preset_days = self.date_range_preset.to_days();
        let date_range = self
            .downloaded_tickers
            .lock()
            .unwrap()
            .get_range(&ticker_info.ticker)
            .unwrap_or_else(|| {
                log::warn!(
                    "No registered range for {} - using preset ({} days)",
                    ticker_info.ticker,
                    preset_days
                );
                DateRange::last_n_days(preset_days)
            });

        log::info!(
            "DASHBOARD: Using date range {} to {} for {}",
            date_range.start,
            date_range.end,
            ticker_info.ticker
        );

        // Get mutable reference to the focused pane state
        let Some(pane_state) = self.get_mut_pane(main_window, window, pane) else {
            log::error!("Focused pane not found in state");
            return Task::done(Message::Notification(Toast::error(
                "Failed to find pane".to_string(),
            )));
        };

        // Set content and trigger chart loading with registered date range
        let effect = pane_state.set_content_with_range(ticker_info, content_kind, date_range);

        // Handle the LoadChart effect
        match effect {
            pane::Effect::LoadChart {
                config,
                ticker_info,
            } => {
                let pane_id = pane_state.unique_id();
                let event = self.load_chart(pane_id, config, ticker_info);

                // Return task that will emit the LoadChart event
                match event {
                    Event::LoadChart {
                        pane_id,
                        config,
                        ticker_info,
                    } => Task::done(Message::LoadChart {
                        pane_id,
                        config,
                        ticker_info,
                    }),
                    Event::Notification(toast) => Task::done(Message::Notification(toast)),
                    Event::EstimateDataCost { .. } | Event::DownloadData { .. } => {
                        // These shouldn't appear from set_content, but handle gracefully
                        Task::none()
                    }
                }
            }
            _ => {
                log::warn!("Unexpected effect from set_content: {:?}", effect);
                Task::none()
            }
        }
    }

    pub fn switch_tickers_in_group(
        &mut self,
        main_window: window::Id,
        ticker_info: FuturesTickerInfo,
        triggering_pane_link_group: Option<LinkGroup>,
        fallback_pane_id: Option<uuid::Uuid>,
    ) -> Task<Message> {
        let mut panes_to_update = Vec::new();

        // If pane has a link group, update ALL panes in that group
        if let Some(link_group) = triggering_pane_link_group {
            log::info!(
                "Switching tickers in link group {:?} to {:?}",
                link_group,
                ticker_info.ticker
            );

            // Collect all panes in this link group
            for (window, pane, state) in self.iter_all_panes(main_window) {
                if state.link_group == Some(link_group) {
                    panes_to_update.push((window, pane, state.unique_id(), state.content.kind()));
                }
            }
        } else if let Some(pane_id) = fallback_pane_id {
            // No link group - just update the single triggering pane
            log::info!(
                "No link group - switching single pane {} to {:?}",
                pane_id,
                ticker_info.ticker
            );

            // Find the pane by UUID
            if let Some((window, pane, state)) = self
                .iter_all_panes(main_window)
                .find(|(_, _, s)| s.unique_id() == pane_id)
            {
                panes_to_update.push((window, pane, state.unique_id(), state.content.kind()));
            } else {
                log::error!("Could not find triggering pane by UUID: {}", pane_id);
                return Task::none();
            }
        } else {
            log::debug!("No link group and no fallback pane ID - cannot switch tickers");
            return Task::none();
        }

        log::info!(
            "Switching {} pane(s) to ticker {:?}",
            panes_to_update.len(),
            ticker_info.ticker
        );

        // Get registered date range BEFORE looping
        let preset_days = self.date_range_preset.to_days();
        let date_range = self
            .downloaded_tickers
            .lock()
            .unwrap()
            .get_range(&ticker_info.ticker)
            .unwrap_or_else(|| {
                log::warn!(
                    "No registered range for {} in switch_tickers_in_group \
                     - using preset ({} days)",
                    ticker_info.ticker,
                    preset_days
                );
                DateRange::last_n_days(preset_days)
            });

        log::info!(
            "Using date range {} to {} for ticker switch",
            date_range.start,
            date_range.end
        );

        // Update each pane's ticker and trigger reload
        let mut tasks = Vec::new();
        for (_, _, pane_id, content_kind) in panes_to_update {
            // Get the pane state and update it
            if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window, pane_id) {
                // Use set_content_with_range to use registered date range
                let effect =
                    pane_state.set_content_with_range(ticker_info, content_kind, date_range);

                log::info!(
                    "  Pane {} effect received: {:?}",
                    pane_id,
                    match &effect {
                        pane::Effect::LoadChart { config, .. } =>
                            format!("LoadChart({:?})", config.chart_type),
                        pane::Effect::SwitchTickersInGroup(_) => "SwitchTickersInGroup".to_string(),
                        _ => "Other".to_string(),
                    }
                );

                // Handle the LoadChart effect
                if let pane::Effect::LoadChart {
                    config,
                    ticker_info,
                } = effect
                {
                    log::info!("  Creating LoadChart event for pane {}", pane_id);
                    let event = self.load_chart(pane_id, config, ticker_info);
                    if let Event::LoadChart {
                        pane_id,
                        config,
                        ticker_info,
                    } = event
                    {
                        log::info!("  Pushing LoadChart message to task queue");
                        tasks.push(Message::LoadChart {
                            pane_id,
                            config,
                            ticker_info,
                        });
                    }
                }
            }
        }

        // Return task that triggers all chart loads
        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks.into_iter().map(Task::done))
        }
    }

    /// Handle loaded chart data for a specific pane
    pub fn handle_chart_data_loaded(
        &mut self,
        main_window: window::Id,
        pane_id: uuid::Uuid,
        chart_data: ChartData,
    ) -> Task<Message> {
        // Update chart state first (separate borrow)
        if let Some(chart_state) = self.charts.get_mut(&pane_id) {
            chart_state.data = chart_data.clone();
            chart_state.loading_status = LoadingStatus::Ready;
        }

        // Update pane state (separate borrow)
        if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window, pane_id) {
            pane_state.set_chart_data(chart_data);
            log::info!("Chart data loaded for pane {}", pane_id);
        } else {
            log::warn!("Pane {} not found for chart data", pane_id);
        }

        Task::none()
    }

    /// Request chart data loading for a pane
    pub fn load_chart(
        &mut self,
        pane_id: uuid::Uuid,
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    ) -> Event {
        // Update loading status
        if let Some(chart_state) = self.charts.get_mut(&pane_id) {
            chart_state.loading_status = LoadingStatus::Building {
                operation: String::new(),
                progress: 0.0,
            };
        } else {
            // Create new chart state
            self.charts.insert(
                pane_id,
                ChartState {
                    config: config.clone(),
                    data: ChartData::from_trades(vec![], vec![]),
                    ticker_info,
                    loading_status: LoadingStatus::Building {
                        operation: String::new(),
                        progress: 0.0,
                    },
                },
            );
        }

        Event::LoadChart {
            pane_id,
            config,
            ticker_info,
        }
    }
}
