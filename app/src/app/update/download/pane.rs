use iced::Task;

use crate::components::display::toast::{Notification, Toast};
use crate::screen::dashboard;
use super::super::super::{Kairos, Message};
use super::super::super::globals::get_download_progress;

impl Kairos {
    pub(crate) fn handle_download_complete(
        &mut self,
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        date_range: data::DateRange,
        result: Result<usize, String>,
    ) -> Task<Message> {
        {
            let mut progress = data::lock_or_recover(get_download_progress());
            progress.remove(&pane_id);
            if progress.is_empty() {
                super::super::super::globals::set_download_active(false);
            }
        }

        match result {
            Ok(days_downloaded) => {
                log::info!(
                    "Downloaded {} days for {} ({} to {})",
                    days_downloaded,
                    ticker,
                    date_range.start,
                    date_range.end
                );
                self.notifications
                    .push(Toast::new(Notification::Info(format!(
                        "Successfully downloaded {} days of data",
                        days_downloaded
                    ))));

                data::lock_or_recover(&self.downloaded_tickers)
                    .register(ticker, date_range);
                log::info!("Registered {} in downloaded tickers registry", ticker);

                // Re-scan cache to rebuild the DataIndex with new data
                let cache_root =
                    crate::infra::platform::data_path(Some("cache/databento"));
                let scan_feed_id = {
                    let fm = self
                        .data_feed_manager
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    fm.connected_feed_id_for_provider(data::FeedProvider::Databento)
                        .unwrap_or(uuid::Uuid::nil())
                };
                let scan_task = Task::perform(
                    async move {
                        exchange::scan_databento_cache(&cache_root, scan_feed_id)
                            .await
                    },
                    Message::DataIndexRebuilt,
                );

                if pane_id == uuid::Uuid::nil() {
                    self.data_management_panel
                        .set_download_progress(crate::modals::download::DownloadProgress::Idle);

                    let estimate_ticker = data::FuturesTicker::new(
                        crate::modals::download::FUTURES_PRODUCTS
                            [self.data_management_panel.selected_ticker_idx()]
                        .0,
                        data::FuturesVenue::CMEGlobex,
                    );
                    let schema = crate::modals::download::SCHEMAS
                        [self.data_management_panel.selected_schema_idx()]
                    .0;
                    let estimate_date_range = self.data_management_panel.current_date_range();

                    return Task::batch([
                        scan_task,
                        Task::done(Message::Download(
                            super::super::super::DownloadMessage::EstimateDataCost {
                                pane_id: uuid::Uuid::nil(),
                                ticker: estimate_ticker,
                                schema,
                                date_range: estimate_date_range,
                            },
                        )),
                    ]);
                } else {
                    let layout_id = self
                        .layout_manager
                        .active_layout_id()
                        .map(|id| id.unique)
                        .or_else(|| self.layout_manager.layouts.first().map(|l| l.id.unique));

                    let Some(layout_id) = layout_id else {
                        log::error!("No layout available for DataDownloadComplete");
                        return scan_task;
                    };

                    return Task::batch([
                        scan_task,
                        Task::done(Message::Dashboard {
                            layout_id: Some(layout_id),
                            event: dashboard::Message::DataDownloadComplete {
                                pane_id,
                                days_downloaded,
                            },
                        }),
                    ]);
                }
            }
            Err(e) => {
                log::error!("Failed to download data: {}", e);
                self.notifications
                    .push(Toast::error(format!("Download failed: {}", e)));
            }
        }
        Task::none()
    }
}
