use super::{Dashboard, Message};
use crate::window;
use data::{ChartConfig, DateRange, LoadingStatus};
use exchange::FuturesTickerInfo;
use iced::Task;
use std::time::Instant;

impl Dashboard {
    /// Unaffiliate panes from a disconnected feed without destroying chart data.
    /// Charts remain visible; panes are just marked as having no active feed.
    /// Returns the number of panes affected.
    pub fn unaffiliate_panes_for_feed(
        &mut self,
        feed_id: data::FeedId,
        main_window: window::Id,
    ) -> usize {
        let mut count = 0;
        for (_, _, state) in self.iter_all_panes_mut(main_window) {
            if state.feed_id == Some(feed_id) {
                state.feed_id = None;
                count += 1;
            }
        }
        if count > 0 {
            log::info!(
                "Unaffiliated {} pane(s) from disconnected feed {}",
                count,
                feed_id
            );
        }
        count
    }

    /// Re-affiliate disconnected panes (feed_id == None) to a new feed,
    /// and collect panes that had a failed load and need reloading.
    /// Returns a list of (pane_id, config, ticker_info) for panes to reload.
    pub fn affiliate_and_collect_reloads(
        &mut self,
        new_feed_id: data::FeedId,
        main_window: window::Id,
    ) -> Vec<(uuid::Uuid, ChartConfig, FuturesTickerInfo)> {
        let mut to_reload = Vec::new();

        // First pass: collect info from panes that need reloading
        let preset_days = self.date_range_preset.to_days();
        for (_, _, state) in self.iter_all_panes(main_window) {
            if state.feed_id.is_none() && state.ticker_info.is_some() {
                let needs_reload = matches!(
                    &state.loading_status,
                    LoadingStatus::Error { .. }
                );
                if needs_reload
                    && let Some(ticker_info) = state.ticker_info {
                        let date_range = self
                            .downloaded_tickers
                            .lock()
                            .unwrap()
                            .get_range(&ticker_info.ticker)
                            .unwrap_or_else(|| {
                                DateRange::last_n_days(preset_days)
                            });

                        let config = ChartConfig {
                            chart_type: state.content.kind().to_chart_type(),
                            basis: state
                                .settings
                                .selected_basis
                                .unwrap_or(data::ChartBasis::Time(
                                    data::Timeframe::M5,
                                )),
                            ticker: ticker_info.ticker,
                            date_range,
                        };
                        to_reload.push((
                            state.unique_id(),
                            config,
                            ticker_info,
                        ));
                    }
            }
        }

        // Second pass: affiliate all disconnected panes
        for (_, _, state) in self.iter_all_panes_mut(main_window) {
            if state.feed_id.is_none() && state.ticker_info.is_some() {
                state.feed_id = Some(new_feed_id);
            }
        }

        to_reload
    }

    pub fn invalidate_all_panes(&mut self, main_window: window::Id) {
        self.iter_all_panes_mut(main_window)
            .for_each(|(_, _, state)| {
                let _ = state.invalidate(Instant::now());
            });
    }

    pub fn tick(&mut self, now: Instant, main_window: window::Id) -> Task<Message> {
        // Tick all panes for canvas invalidation and animations
        self.iter_all_panes_mut(main_window)
            .for_each(|(_window_id, _pane, state)| {
                // Just invalidate charts for rendering updates
                let _ = state.invalidate(now);
            });

        Task::none()
    }
}
