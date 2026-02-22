use iced::Task;

use super::super::super::{Kairos, Message};

impl Kairos {
    pub(super) fn handle_feeds_updated(
        &mut self,
        feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        log::info!("Data feeds updated, persisting to disk");
        drop(feed_manager);
        let windows = std::collections::HashMap::new();
        self.save_state_to_disk(&windows);
        let feed_manager = self
            .data_feed_manager
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.data_feeds_modal.sync_snapshot(&feed_manager);
        self.connections_menu.sync_snapshot(&feed_manager);
        Task::none()
    }
}
