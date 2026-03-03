/// Create a StudyRegistry with all native studies.
///
/// Call this instead of `StudyRegistry::new()` to get a registry with all
/// available indicators.
pub(crate) fn create_unified_registry() -> study::StudyRegistry {
    study::StudyRegistry::new()
}

/// Result of DataEngine initialization.
///
/// Wraps the engine and event receiver in Arc so that the result can be
/// carried through the Iced `Message` enum (which requires `Clone`).
/// The engine is wrapped in `Arc<tokio::sync::Mutex<>>` for shared ownership;
/// the event receiver is wrapped in `Arc<std::sync::Mutex<Option<>>>` so the
/// subscription stream can take it exactly once.
#[derive(Clone)]
pub(crate) struct DataEngineInit {
    pub(crate) engine: std::sync::Arc<tokio::sync::Mutex<data::engine::DataEngine>>,
    pub(crate) event_rx: std::sync::Arc<
        std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<data::DataEvent>>>,
    >,
    /// Event sender extracted from the engine *before* it is wrapped in a
    /// Mutex, so the sync Iced event loop never needs to lock the engine.
    pub(crate) event_tx: tokio::sync::mpsc::UnboundedSender<data::DataEvent>,
    pub(crate) server_resolver: Option<std::sync::Arc<data::ServerResolver>>,
}

impl std::fmt::Debug for DataEngineInit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataEngineInit").finish_non_exhaustive()
    }
}

/// Initialize the DataEngine asynchronously — called via Task::perform at startup.
///
/// If a Databento API key is provided, the adapter is connected eagerly so
/// that downloads and cost estimation work immediately without a separate
/// connect step.
pub(crate) async fn initialize_data_engine(
    databento_key: Option<String>,
) -> Result<DataEngineInit, String> {
    let cache_root = crate::infra::platform::data_path(Some("cache"));

    // Resolve Rithmic server URLs (non-fatal — Databento-only users unaffected)
    let data_dir = crate::infra::platform::data_path(None);
    let server_resolver = match data::ServerResolver::initialize(data_dir).await {
        Ok(resolver) => {
            log::info!("Rithmic server resolver initialized");
            Some(std::sync::Arc::new(resolver))
        }
        Err(e) => {
            log::warn!("Server resolver init (non-fatal): {}", e);
            None
        }
    };

    match data::engine::DataEngine::new(cache_root).await {
        Ok((mut engine, event_rx)) => {
            log::info!("DataEngine initialized successfully");

            // Eagerly connect Databento adapter when an API key is available
            if let Some(key) = databento_key {
                let config = data::DatabentoConfig::with_api_key(key);
                if let Err(e) = engine.connect_databento(config).await {
                    log::warn!("Databento adapter init failed (non-fatal): {}", e);
                }
            }

            // Extract the event sender BEFORE wrapping in Mutex so the
            // sync Iced event loop never needs blocking_lock().
            let event_tx = engine.event_sender();

            Ok(DataEngineInit {
                engine: std::sync::Arc::new(tokio::sync::Mutex::new(engine)),
                event_rx: std::sync::Arc::new(std::sync::Mutex::new(Some(event_rx))),
                event_tx,
                server_resolver,
            })
        }
        Err(e) => {
            log::error!("Failed to initialize DataEngine: {}", e);
            Err(e.to_string())
        }
    }
}
