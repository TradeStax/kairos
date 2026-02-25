pub(crate) mod ai;
mod backtest;
mod chart;
mod download;
mod feeds;
pub(crate) mod menu_bar;
mod shell;
#[cfg(feature = "options")]
mod options;
mod preferences;
mod replay;

use iced::Task;

use crate::components::display::toast::Toast;

#[cfg(feature = "options")]
use super::OptionsMessage;
use super::{ChartMessage, DownloadMessage, Kairos, Message, WindowMessage, build_tickers_info};

impl Kairos {
    /// Lock the DataFeedManager and call `f` with a mutable reference.
    /// Avoids the clone-Arc → lock → drop → clone-Arc → re-lock pattern.
    pub(crate) fn with_feed_manager<R>(
        &self,
        f: impl FnOnce(&mut data::DataFeedManager) -> R,
    ) -> R {
        self.connections.with_feed_manager(f)
    }

    /// Rebuild `tickers_info` and `ticker_ranges` from the current DataIndex.
    pub(crate) fn rebuild_ticker_data(&mut self) {
        let tickers: std::collections::HashSet<String> =
            data::lock_or_recover(&self.persistence.data_index)
                .available_tickers()
                .into_iter()
                .collect();
        self.persistence.tickers_info = build_tickers_info(tickers);
        self.persistence.ticker_ranges =
            Self::build_ticker_ranges(&self.persistence.data_index);
    }

    /// Sync both feed modal snapshots from the current DataFeedManager state.
    pub(crate) fn sync_feed_snapshots(
        &mut self,
        feed_manager: &data::DataFeedManager,
    ) {
        self.modals.data_feeds_modal.sync_snapshot(feed_manager);
        self.modals.connections_menu.sync_snapshot(feed_manager);
    }

    /// Get market data service or push error toast and return None.
    pub(crate) fn require_market_service(
        &mut self,
    ) -> Option<std::sync::Arc<data::MarketDataService>> {
        if let Some(service) = self.services.market_data_service.clone() {
            Some(service)
        } else {
            log::warn!(
                "Market data service not available (API key not configured)"
            );
            self.ui.notifications.push(Toast::error(
                "Databento API key not configured. Set it in connection \
                 settings."
                    .to_string(),
            ));
            None
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // Chart data loading (sub-enum)
            Message::Chart(msg) => match msg {
                ChartMessage::LoadChartData {
                    layout_id,
                    pane_id,
                    config,
                    ticker_info,
                } => {
                    return self.handle_load_chart_data(layout_id, pane_id, config, ticker_info);
                }
                ChartMessage::ChartDataLoaded {
                    layout_id,
                    pane_id,
                    result,
                } => {
                    return self.handle_chart_data_loaded(layout_id, pane_id, result);
                }
                ChartMessage::UpdateLoadingStatus => {
                    return self.fetch_loading_statuses();
                }
                ChartMessage::LoadingStatusesReady(statuses) => {
                    return self.dispatch_loading_statuses(statuses);
                }
            },

            // Options data loading (sub-enum)
            #[cfg(feature = "options")]
            Message::Options(msg) => match msg {
                OptionsMessage::OptionChainLoaded { pane_id, result } => {
                    self.handle_option_chain_loaded(pane_id, result);
                }
                OptionsMessage::GexProfileLoaded { pane_id, result } => {
                    self.handle_gex_profile_loaded(pane_id, result);
                }
            },

            // Data download (sub-enum)
            Message::Download(msg) => match msg {
                DownloadMessage::EstimateDataCost {
                    pane_id,
                    ticker,
                    schema,
                    date_range,
                } => {
                    return self.handle_estimate_data_cost(pane_id, ticker, schema, date_range);
                }
                DownloadMessage::DataCostEstimated { pane_id, result } => {
                    return self.handle_data_cost_estimated(pane_id, result);
                }
                DownloadMessage::DownloadData {
                    pane_id,
                    ticker,
                    schema,
                    date_range,
                } => {
                    return self.handle_download_data(pane_id, ticker, schema, date_range);
                }
                DownloadMessage::DataDownloadProgress {
                    pane_id,
                    current,
                    total,
                } => {
                    return self.handle_download_progress(pane_id, current, total);
                }
                DownloadMessage::DataDownloadComplete {
                    pane_id,
                    ticker,
                    date_range,
                    result,
                } => {
                    return self.handle_download_complete(pane_id, ticker, date_range, result);
                }
                DownloadMessage::ApiKeySetup(msg) => {
                    return self.handle_api_key_setup(msg);
                }
                DownloadMessage::HistoricalDownload(msg) => {
                    return self.handle_historical_download(msg);
                }
                DownloadMessage::HistoricalDownloadCostEstimated { result } => {
                    self.handle_historical_download_cost_estimated(result);
                }
                DownloadMessage::HistoricalDownloadComplete {
                    ticker,
                    date_range,
                    result,
                } => {
                    self.handle_historical_download_complete(ticker, date_range, result);
                }
            },

            // Data index rebuilt after cache scan
            Message::DataIndexRebuilt(result) => {
                return self.handle_data_index_rebuilt(result);
            }

            // Data feeds and connections
            Message::DataFeeds(msg) => {
                return self.handle_data_feeds(msg);
            }
            Message::DataFeedPreviewLoaded { feed_id, result } => {
                self.handle_data_feed_preview_loaded(feed_id, result);
            }
            Message::ConnectionsMenu(msg) => {
                return self.handle_connections_menu(msg);
            }
            Message::RithmicConnected { feed_id, result } => {
                return self.handle_rithmic_connected(feed_id, result);
            }
            Message::RithmicSystemNames { server, result } => {
                self.modals.data_feeds_modal.update(
                    crate::modals::data_feeds::DataFeedsMessage::SystemNamesLoaded(
                        server, result,
                    ),
                    &mut data::lock_or_recover(
                        &self.connections.data_feed_manager,
                    ),
                );
            }
            Message::RithmicProductCodes {
                feed_id: _,
                result,
            } => {
                self.modals.data_feeds_modal.update(
                    crate::modals::data_feeds::DataFeedsMessage::AvailableTickersLoaded(
                        result,
                    ),
                    &mut data::lock_or_recover(
                        &self.connections.data_feed_manager,
                    ),
                );
            }
            Message::RithmicStreamEvent(event) => {
                return self.handle_rithmic_stream_event(event);
            }
            Message::Replay(msg) => {
                return self.handle_replay_message(msg);
            }
            Message::ReplayEvent(event) => {
                return self.handle_replay_event(event);
            }
            Message::Backtest(msg) => {
                return self.handle_backtest_message(msg);
            }
            Message::MenuBar(msg) => {
                return self.handle_menu_bar(msg);
            }

            // Window/navigation events
            Message::Tick(now) => {
                return self.handle_tick(now);
            }
            Message::WindowEvent(event) => {
                return self.handle_window_event(event);
            }
            Message::ExitRequested(windows) => {
                return self.handle_exit_requested(windows);
            }
            Message::GoBack => {
                self.handle_go_back();
            }
            Message::Dashboard {
                layout_id: id,
                event: msg,
            } => {
                return self.handle_dashboard(id, msg);
            }
            Message::DataFolderRequested => {
                self.handle_data_folder_requested();
            }

            // Preferences and UI
            Message::ThemeSelected(theme) => {
                self.handle_theme_selected(theme);
            }
            Message::ScaleFactorChanged(value) => {
                self.handle_scale_factor_changed(value);
            }
            Message::SetTimezone(tz) => {
                self.handle_set_timezone(tz);
            }
            Message::RemoveNotification(index) => {
                self.handle_remove_notification(index);
            }
            Message::ToggleDialogModal(dialog) => {
                self.handle_toggle_dialog_modal(dialog);
            }
            Message::ReinitializeService(provider) => {
                return self.handle_reinitialize_service(provider);
            }
            Message::Layouts(message) => {
                return self.handle_layouts(message);
            }
            Message::ThemeEditor(msg) => {
                self.handle_theme_editor(msg);
            }
            Message::Sidebar(message) => {
                return self.handle_sidebar(message);
            }

            // Window control messages (custom title bar)
            Message::Window(msg) => match msg {
                WindowMessage::TitleBarHover(hovered) => {
                    self.ui.title_bar_hovered = hovered;
                }
                WindowMessage::Drag(id) => {
                    return self.handle_window_drag(id);
                }
                WindowMessage::Minimize(id) => {
                    return self.handle_window_minimize(id);
                }
                WindowMessage::ToggleMaximize(id) => {
                    return self.handle_window_toggle_maximize(id);
                }
                WindowMessage::Close(id) => {
                    return self.handle_window_close(id);
                }
            },
            Message::ServicesReady(result) => {
                return self.handle_services_ready(result);
            }
            Message::AiStreamEvent(event) => {
                return self.handle_ai_stream_event(event);
            }
            Message::AiStreamComplete => {
                return self.handle_ai_stream_complete();
            }
        }
        Task::none()
    }
}
