use std::sync::Arc;
use data::{SecretsManager, ApiProvider, ApiKeyStatus};

/// Initialize options services from environment or keyring
pub fn initialize_options_services() -> (
    Option<Arc<data::services::OptionsDataService>>,
    Arc<data::services::GexCalculationService>,
) {
    // GEX service is always available (no I/O, pure computation)
    let gex_service = Arc::new(data::services::GexCalculationService::new());

    // Try to get Massive API key from keyring or environment
    let secrets = SecretsManager::new();
    let api_key_status = secrets.get_api_key(ApiProvider::Massive);

    let options_service = match api_key_status {
        ApiKeyStatus::FromKeyring(api_key) | ApiKeyStatus::FromEnv(api_key) => {
            let source = match secrets.get_api_key(ApiProvider::Massive) {
                ApiKeyStatus::FromKeyring(_) => "keyring",
                ApiKeyStatus::FromEnv(_) => "environment",
                _ => "unknown",
            };
            log::info!("Massive API key found (from {}), initializing options data service", source);

            let config = exchange::MassiveConfig::new(api_key);

            // Initialize repositories asynchronously
            match tokio::runtime::Runtime::new() {
                Ok(rt) => {
                    rt.block_on(async {
                        let snapshot_repo_result = exchange::MassiveSnapshotRepository::new(config.clone()).await;
                        let chain_repo_result = exchange::MassiveChainRepository::new(config.clone()).await;
                        let contract_repo_result = exchange::MassiveContractRepository::new(config).await;

                        match (snapshot_repo_result, chain_repo_result, contract_repo_result) {
                            (Ok(snapshot_repo), Ok(chain_repo), Ok(contract_repo)) => {
                                let service = data::services::OptionsDataService::new(
                                    Arc::new(snapshot_repo),
                                    Arc::new(chain_repo),
                                    Arc::new(contract_repo),
                                );

                                log::info!("✓ Options data service initialized successfully");
                                Some(Arc::new(service))
                            }
                            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                                log::error!("Failed to initialize options repositories: {}", e);
                                None
                            }
                        }
                    })
                }
                Err(e) => {
                    log::error!("Failed to create runtime for options service: {}", e);
                    None
                }
            }
        }
        ApiKeyStatus::NotConfigured => {
            log::info!("Massive API key not configured - options data features disabled");
            log::info!("Configure via Settings > API Keys or set MASSIVE_API_KEY environment variable");
            None
        }
    };

    (options_service, gex_service)
}

/// Result of market data service initialization
pub struct MarketDataServiceResult {
    pub service: Arc<data::MarketDataService>,
    pub trade_repo: Arc<exchange::DatabentoTradeRepository>,
    pub depth_repo: Arc<exchange::DatabentoDepthRepository>,
}

/// Initialize market data repositories and service
/// Returns None if API key is not configured
pub fn initialize_market_data_service() -> Option<MarketDataServiceResult> {
    // Try to get Databento API key from keyring or environment
    let secrets = SecretsManager::new();
    let api_key_status = secrets.get_api_key(ApiProvider::Databento);

    let databento_config = match api_key_status {
        ApiKeyStatus::FromKeyring(api_key) => {
            log::info!("Databento API key found in keyring");
            exchange::adapter::databento::DatabentoConfig::with_api_key(api_key)
        }
        ApiKeyStatus::FromEnv(api_key) => {
            log::info!("Databento API key found in environment");
            exchange::adapter::databento::DatabentoConfig::with_api_key(api_key)
        }
        ApiKeyStatus::NotConfigured => {
            log::warn!("Databento API key not configured - market data features disabled");
            log::info!("Configure via Settings > API Keys or set DATABENTO_API_KEY environment variable");
            return None;
        }
    };

    // Create runtime for async repository initialization
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            log::error!("Failed to create runtime for market data service: {}", e);
            return None;
        }
    };

    // Create repository instances (async)
    

    rt.block_on(async {
        let trade_result = exchange::DatabentoTradeRepository::new(databento_config.clone()).await;
        let depth_result = exchange::DatabentoDepthRepository::new(databento_config).await;

        match (trade_result, depth_result) {
            (Ok(trade), Ok(depth)) => {
                let trade_repo = Arc::new(trade);
                let depth_repo = Arc::new(depth);
                let service = Arc::new(
                    data::MarketDataService::new(trade_repo.clone(), depth_repo.clone())
                );
                log::info!("Market data service initialized successfully");
                Some(MarketDataServiceResult {
                    service,
                    trade_repo,
                    depth_repo,
                })
            }
            (Err(e), _) => {
                log::error!("Failed to create trade repository: {}", e);
                None
            }
            (_, Err(e)) => {
                log::error!("Failed to create depth repository: {}", e);
                None
            }
        }
    })
}

/// Create replay engine for historical data playback
/// Returns None if market data result is not available
pub fn create_replay_engine(
    market_data_result: Option<&MarketDataServiceResult>,
) -> Option<Arc<std::sync::Mutex<data::services::ReplayEngine>>> {
    let result = market_data_result?;

    // Convert concrete types to trait objects
    let depth_repo_dyn: Option<Arc<dyn data::DepthRepository + Send + Sync>> =
        Some(result.depth_repo.clone() as Arc<dyn data::DepthRepository + Send + Sync>);

    Some(Arc::new(std::sync::Mutex::new(
        data::services::ReplayEngine::with_default_config(result.trade_repo.clone(), depth_repo_dyn)
    )))
}
