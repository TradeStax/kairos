mod historical;
mod pane;

use iced::Task;

use crate::components::display::toast::Toast;
use crate::screen::dashboard;
use super::super::{DownloadMessage, Kairos, Message};
use super::super::globals::get_download_progress;

impl Kairos {
    pub(crate) fn handle_estimate_data_cost(
        &mut self,
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DownloadSchema,
        date_range: data::DateRange,
    ) -> Task<Message> {
        log::info!("EstimateDataCost message received");
        log::info!(
            "Ticker={:?}, Schema={:?}, Range={:?}",
            ticker,
            schema,
            date_range
        );

        let Some(service) = self.market_data_service.clone() else {
            log::warn!("Market data service not available (API key not configured)");
            self.notifications.push(Toast::error(
                "Databento API key not configured. Set it in connection \
                 settings."
                    .to_string(),
            ));
            return Task::none();
        };

        let schema_discriminant = schema.as_discriminant();
        Task::perform(
            async move {
                log::info!("Async block entered, about to call service");
                let result = service
                    .estimate_data_request(&ticker, schema_discriminant, &date_range)
                    .await;
                log::info!("Service call completed, result success: {}", result.is_ok());
                if let Err(ref e) = result {
                    log::error!("Service error: {}", e);
                }
                result.map_err(|e| e.to_string())
            },
            move |result| {
                log::info!("Task finished, sending DataCostEstimated");
                Message::Download(DownloadMessage::DataCostEstimated { pane_id, result })
            },
        )
    }

    pub(crate) fn handle_data_cost_estimated(
        &mut self,
        pane_id: uuid::Uuid,
        result: Result<data::DataRequestEstimate, String>,
    ) -> Task<Message> {
        match result {
            Ok(estimate) => {
                let cached_days = estimate.cached_dates.len();
                log::info!(
                    "Cost estimated: {}/{} days cached, ${:.4} USD",
                    cached_days,
                    estimate.total_days,
                    estimate.estimated_cost_usd
                );

                if pane_id == uuid::Uuid::nil() {
                    self.data_management_panel.set_cache_status(
                        crate::modals::download::CacheStatus {
                            total_days: estimate.total_days,
                            cached_days,
                            uncached_days: estimate.uncached_count,
                            gaps_description: None,
                        },
                        estimate.cached_dates.clone(),
                    );
                    self.data_management_panel
                        .set_actual_cost(estimate.estimated_cost_usd);
                } else {
                    log::info!(
                        "Cost estimated for pane {}: {}/{} days cached",
                        pane_id,
                        cached_days,
                        estimate.total_days
                    );
                }
            }
            Err(e) => {
                log::error!("Failed to estimate cost: {}", e);
                self.notifications
                    .push(Toast::error(format!("Estimation failed: {}", e)));
            }
        }
        Task::none()
    }

    pub(crate) fn handle_download_data(
        &mut self,
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DownloadSchema,
        date_range: data::DateRange,
    ) -> Task<Message> {
        let Some(service) = self.market_data_service.clone() else {
            log::warn!("Market data service not available (API key not configured)");
            self.notifications.push(Toast::error(
                "Databento API key not configured. Set it in connection \
                 settings."
                    .to_string(),
            ));
            return Task::none();
        };

        let schema_discriminant = schema.as_discriminant();
        let ticker_clone = ticker;
        let date_range_clone = date_range;

        {
            let mut progress = data::lock_or_recover(get_download_progress());
            progress.insert(pane_id, (0, date_range.num_days() as usize));
        }
        super::super::globals::set_download_active(true);

        Task::perform(
            async move {
                service
                    .download_to_cache_with_progress(
                        &ticker,
                        schema_discriminant,
                        &date_range,
                        Box::new(move |current, total| {
                            if let Ok(mut progress) = get_download_progress().lock() {
                                progress.insert(pane_id, (current, total));
                            }
                            log::info!("Download progress: {}/{} days", current, total);
                        }),
                    )
                    .await
                    .map_err(|e| e.to_string())
            },
            move |result| {
                Message::Download(DownloadMessage::DataDownloadComplete {
                    pane_id,
                    ticker: ticker_clone,
                    date_range: date_range_clone,
                    result,
                })
            },
        )
    }

    pub(crate) fn handle_download_progress(
        &mut self,
        pane_id: uuid::Uuid,
        current: usize,
        total: usize,
    ) -> Task<Message> {
        log::info!(
            "Download progress for pane {}: {}/{}",
            pane_id,
            current,
            total
        );

        if self.historical_download_id == Some(pane_id) {
            if let Some(modal) = &mut self.historical_download_modal {
                modal.set_download_progress(
                    crate::modals::download::DownloadProgress::Downloading {
                        current_day: current,
                        total_days: total,
                    },
                );
            }
        } else if pane_id == uuid::Uuid::nil() {
            self.data_management_panel.set_download_progress(
                crate::modals::download::DownloadProgress::Downloading {
                    current_day: current,
                    total_days: total,
                },
            );
        } else {
            return Task::done(Message::Dashboard {
                layout_id: self.layout_manager.active_layout_id().map(|l| l.unique),
                event: dashboard::Message::DataDownloadProgress {
                    pane_id,
                    current,
                    total,
                },
            });
        }
        Task::none()
    }
}
