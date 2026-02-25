//! Live connection state: Rithmic client, trade/depth repos, data feed manager.

use data::FeedId;

pub(crate) struct ConnectionState {
    pub(crate) rithmic_client:
        Option<std::sync::Arc<tokio::sync::Mutex<exchange::RithmicClient>>>,
    pub(crate) rithmic_trade_repo:
        Option<std::sync::Arc<exchange::RithmicTradeRepository>>,
    pub(crate) rithmic_depth_repo:
        Option<std::sync::Arc<exchange::RithmicDepthRepository>>,
    pub(crate) rithmic_feed_id: Option<FeedId>,
    pub(crate) rithmic_reconnect_attempts: u32,
    pub(crate) data_feed_manager:
        std::sync::Arc<std::sync::Mutex<data::DataFeedManager>>,
}

impl ConnectionState {
    pub(crate) fn new(
        data_feed_manager: std::sync::Arc<std::sync::Mutex<data::DataFeedManager>>,
    ) -> Self {
        Self {
            rithmic_client: None,
            rithmic_trade_repo: None,
            rithmic_depth_repo: None,
            rithmic_feed_id: None,
            rithmic_reconnect_attempts: 0,
            data_feed_manager,
        }
    }

    /// Lock the DataFeedManager and call `f`.
    ///
    /// BORROW SAFETY: This method takes `&self`. If the caller needs to mutate other
    /// fields of the parent struct after calling this, the borrow must not overlap.
    pub(crate) fn with_feed_manager<R>(
        &self,
        f: impl FnOnce(&mut data::DataFeedManager) -> R,
    ) -> R {
        let mut guard = data::lock_or_recover(&self.data_feed_manager);
        f(&mut *guard)
    }
}
