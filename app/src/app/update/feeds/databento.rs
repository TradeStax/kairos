use iced::Task;

use crate::app::{Kairos, Message};
use crate::components::display::toast::Toast;

impl Kairos {
    pub(super) fn connect_databento_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        if !self.secrets.has_api_key(data::config::secrets::ApiProvider::Databento) {
            self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
            self.ui.notifications.push(Toast::error(
                "Databento API key not configured. Set it in connection \
                 settings."
                    .to_string(),
            ));
            return Task::none();
        }

        // Immediately seed DataIndex from feed's dataset info (if available)
        // so tickers and ranges are available before the async scan completes.
        if let Some(feed) = feed_manager.get(feed_id) {
            if let Some(info) = feed.dataset_info() {
                let mut dates = std::collections::BTreeSet::new();
                for d in info.date_range.dates() {
                    dates.insert(d);
                }
                let mut idx = data::lock_or_recover(&self.persistence.data_index);
                idx.add_contribution(
                    data::DataKey {
                        ticker: info.ticker.clone(),
                        schema: "trades".to_string(),
                    },
                    feed_id,
                    dates,
                    false,
                );
                drop(idx);

                // Rebuild tickers_info and ticker_ranges immediately
                self.rebuild_ticker_data();
            }
        }

        feed_manager.set_status(feed_id, data::FeedStatus::Connected);
        self.sync_feed_snapshots(&feed_manager);
        drop(feed_manager);

        log::info!("Databento feed connected - triggering cache scan");

        // Scan the Databento cache to build the DataIndex
        let cache_root = crate::infra::platform::data_path(Some("cache/databento"));
        Task::perform(
            async move { exchange::scan_databento_cache(&cache_root, feed_id).await },
            Message::DataIndexRebuilt,
        )
    }

    pub(super) fn disconnect_databento_feed(
        &mut self,
        feed_id: data::FeedId,
        feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        // Remove this feed's contributions from the shared DataIndex
        data::lock_or_recover(&self.persistence.data_index).remove_feed(feed_id);
        self.rebuild_ticker_data();

        // Check if another Databento feed is still connected
        let alt_feed_id =
            feed_manager.connected_feed_id_for_provider(data::FeedProvider::Databento);

        self.sync_feed_snapshots(&feed_manager);
        drop(feed_manager);

        let main_window = self.main_window.id;
        if let Some(alt_fid) = alt_feed_id {
            // Another Databento feed is connected - silently re-affiliate
            for layout in &mut self.persistence.layout_manager.layouts {
                let reloads = layout
                    .dashboard
                    .affiliate_and_collect_reloads(alt_fid, main_window);
                if !reloads.is_empty() {
                    log::info!("Re-affiliated panes to alt feed {}", alt_fid);
                }
            }
        } else {
            // No other feed connected - keep charts visible but
            // mark panes as unaffiliated
            for layout in &mut self.persistence.layout_manager.layouts {
                layout
                    .dashboard
                    .unaffiliate_panes_for_feed(feed_id, main_window);
            }
        }
        Task::none()
    }
}
