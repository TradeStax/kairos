//! Optional service handles: DataEngine, Rithmic client, replay engine.

pub(crate) struct DataEngineState {
    pub(crate) engine: Option<std::sync::Arc<tokio::sync::Mutex<data::engine::DataEngine>>>,
    pub(crate) _event_rx: std::sync::Arc<
        std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<data::DataEvent>>>,
    >,
    /// Clone of the DataEngine's event sender, stored at init time so we can
    /// pass it to streaming tasks without locking the engine.
    pub(crate) event_tx: Option<tokio::sync::mpsc::UnboundedSender<data::DataEvent>>,
    pub(crate) rithmic_client: Option<std::sync::Arc<tokio::sync::Mutex<data::RithmicClient>>>,
    pub(crate) rithmic_feed_id: Option<data::FeedId>,
    pub(crate) rithmic_reconnect_attempts: u32,
    pub(crate) replay_engine:
        Option<std::sync::Arc<tokio::sync::Mutex<crate::services::ReplayEngine>>>,
}

impl DataEngineState {
    pub(crate) fn new() -> Self {
        Self {
            engine: None,
            _event_rx: std::sync::Arc::new(std::sync::Mutex::new(None)),
            event_tx: None,
            rithmic_client: None,
            rithmic_feed_id: None,
            rithmic_reconnect_attempts: 0,
            replay_engine: None,
        }
    }
}
