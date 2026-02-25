//! Optional service handles: market data, options data service, replay engine.

pub(crate) struct ServiceState {
    pub(crate) market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
    #[cfg(feature = "options")]
    pub(crate) options_service:
        Option<std::sync::Arc<data::services::OptionsDataService>>,
    pub(crate) replay_engine:
        Option<std::sync::Arc<tokio::sync::Mutex<data::services::ReplayEngine>>>,
}

impl ServiceState {
    pub(crate) fn new() -> Self {
        Self {
            market_data_service: None,
            #[cfg(feature = "options")]
            options_service: None,
            replay_engine: None,
        }
    }
}
