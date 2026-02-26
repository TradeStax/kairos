pub(crate) mod ai;
mod backtest;
mod chart;
mod data_events;
mod download;
mod feeds;
pub(crate) mod menu_bar;
mod preferences;
mod replay;
mod shell;

use iced::Task;

use crate::components::display::toast::Toast;

use super::{ChartMessage, DownloadMessage, Kairos, Message, WindowMessage, build_tickers_info};

impl Kairos {
    /// Rebuild `tickers_info` and `ticker_ranges` from the current DataIndex.
    pub(crate) fn rebuild_ticker_data(&mut self) {
        let tickers: std::collections::HashSet<String> =
            data::lock_or_recover(&self.persistence.data_index)
                .available_tickers()
                .into_iter()
                .collect();
        self.persistence.tickers_info = build_tickers_info(tickers);
        self.persistence.ticker_ranges = Self::build_ticker_ranges(&self.persistence.data_index);
    }

    /// Sync both feed modal snapshots from the current ConnectionManager state.
    pub(crate) fn sync_feed_snapshots(&mut self, connection_manager: &data::ConnectionManager) {
        self.modals
            .data_feeds_modal
            .sync_snapshot(connection_manager);
        self.modals
            .connections_menu
            .sync_snapshot(connection_manager);
    }

    /// Get the DataEngine or push error toast and return None.
    pub(crate) fn require_data_engine(
        &mut self,
    ) -> Option<std::sync::Arc<tokio::sync::Mutex<data::engine::DataEngine>>> {
        if let Some(engine) = self.services.engine.clone() {
            Some(engine)
        } else {
            log::warn!("DataEngine not available (initialization not complete)");
            self.ui.push_notification(Toast::error(
                "Data engine not ready. Check your connection settings.".to_string(),
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
                    ticker_info,
                    result,
                } => {
                    return self.handle_chart_data_loaded(
                        layout_id, pane_id, ticker_info, result,
                    );
                }
                ChartMessage::UpdateLoadingStatus => {
                    return self.fetch_loading_statuses();
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
                    return self.handle_historical_download_complete(ticker, date_range, result);
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
            Message::ConnectionsMenu(msg) => {
                return self.handle_connections_menu(msg);
            }
            Message::RithmicConnected { feed_id, result } => {
                return self.handle_rithmic_connected(feed_id, result);
            }
            Message::RithmicSystemNames { server, result } => {
                self.handle_rithmic_system_names(server, result);
            }
            Message::RithmicProductCodes {
                _feed_id: _,
                result,
            } => {
                self.handle_rithmic_product_codes(result);
            }
            Message::DataEvent(event) => {
                return self.handle_data_event(event);
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
                return self.handle_dashboard(id, *msg);
            }
            Message::DataFolderRequested => {
                self.handle_data_folder_requested();
            }

            // Preferences and UI
            Message::RemoveNotification(index) => {
                self.handle_remove_notification(index);
            }
            Message::ToggleDialogModal(dialog) => {
                self.handle_toggle_dialog_modal(dialog);
            }
            Message::ReinitializeService(provider) => {
                return self.handle_reinitialize_service(provider);
            }
            Message::ThemeEditor(msg) => {
                self.handle_theme_editor(msg);
            }
            Message::CacheManagement(msg) => {
                return self.handle_cache_management(msg);
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
            Message::DataEngineReady(result) => {
                return self.handle_data_engine_ready(result);
            }
            Message::AiStreamEvent(event) => {
                return self.handle_ai_stream_event(event);
            }
            Message::AiStreamComplete => {
                return self.handle_ai_stream_complete();
            }
            Message::PersistState(windows) => {
                self.save_state_to_disk(&windows);
            }
        }
        Task::none()
    }
}
