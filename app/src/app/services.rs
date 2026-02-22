use crate::infra::secrets::SecretsManager;
use data::config::secrets::{ApiKeyStatus, ApiProvider};
use std::sync::{Arc, Mutex, OnceLock};

/// Global script registry (initialized once, reloadable via Mutex).
static SCRIPT_REGISTRY: OnceLock<Mutex<script::ScriptRegistry>> = OnceLock::new();

/// Initialize the script engine and registry.
///
/// Discovers all bundled and user scripts, compiles them,
/// and stores the registry in a global `OnceLock<Mutex<>>` for reuse.
pub fn initialize_script_registry() {
    SCRIPT_REGISTRY.get_or_init(|| {
        match script::ScriptEngine::new() {
            Ok(mut engine) => {
                let registry = script::ScriptRegistry::new(&mut engine);
                let count = registry.list().len();
                log::info!("Script engine initialized: {} indicators loaded", count);
                Mutex::new(registry)
            }
            Err(e) => {
                log::error!("Failed to initialize script engine: {}", e);
                // Return empty registry on failure - native studies still work
                Mutex::new(script::ScriptRegistry::empty())
            }
        }
    });
}

/// Reload the script registry after scripts are saved/created.
///
/// Re-discovers and compiles all scripts, replacing the existing registry.
pub fn reload_script_registry() {
    if let Some(mutex) = SCRIPT_REGISTRY.get() {
        match script::ScriptEngine::new() {
            Ok(mut engine) => {
                let new_reg = script::ScriptRegistry::new(&mut engine);
                let count = new_reg.list().len();
                log::info!("Script registry reloaded: {} indicators", count);
                if let Ok(mut guard) = mutex.lock() {
                    *guard = new_reg;
                }
            }
            Err(e) => {
                log::error!("Failed to reload script registry: {}", e);
            }
        }
    }
}

/// Create a unified StudyRegistry that includes both native and scripted studies.
///
/// Call this instead of `StudyRegistry::new()` to get a registry with all
/// available indicators.
pub fn create_unified_registry() -> study::StudyRegistry {
    let mut registry = study::StudyRegistry::new();
    if let Some(mutex) = SCRIPT_REGISTRY.get() {
        if let Ok(guard) = mutex.lock() {
            guard.register_into(&mut registry);
        }
    }
    registry
}

/// Initialize options services from environment or keyring
#[cfg(feature = "options")]
pub async fn initialize_options_services() -> (
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
            log::info!(
                "Massive API key found (from {}), initializing options data service",
                source
            );

            let config = exchange::MassiveConfig::new(api_key);

            let snapshot_repo_result =
                exchange::MassiveSnapshotRepository::new(config.clone()).await;
            let chain_repo_result =
                exchange::MassiveChainRepository::new(config.clone()).await;
            let contract_repo_result =
                exchange::MassiveContractRepository::new(config).await;

            match (
                snapshot_repo_result,
                chain_repo_result,
                contract_repo_result,
            ) {
                (Ok(snapshot_repo), Ok(chain_repo), Ok(contract_repo)) => {
                    let service = data::services::OptionsDataService::new(
                        Arc::new(snapshot_repo),
                        Arc::new(chain_repo),
                        Arc::new(contract_repo),
                    );

                    log::info!("Options data service initialized successfully");
                    Some(Arc::new(service))
                }
                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                    log::error!("Failed to initialize options repositories: {}", e);
                    None
                }
            }
        }
        ApiKeyStatus::NotConfigured => {
            log::info!("Massive API key not configured - options data features disabled");
            log::info!(
                "Configure via Settings > API Keys or set MASSIVE_API_KEY environment variable"
            );
            None
        }
    };

    (options_service, gex_service)
}

/// Result of market data service initialization
#[allow(dead_code)]
#[derive(Clone)]
pub struct MarketDataServiceResult {
    pub service: Arc<data::MarketDataService>,
    pub trade_repo: Arc<exchange::DatabentoTradeRepository>,
    pub depth_repo: Arc<exchange::DatabentoDepthRepository>,
}

/// Initialize market data repositories and service
/// Returns None if API key is not configured
pub async fn initialize_market_data_service() -> Option<MarketDataServiceResult> {
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
            log::info!(
                "Configure via Settings > API Keys or set DATABENTO_API_KEY environment variable"
            );
            return None;
        }
    };

    let trade_result = exchange::DatabentoTradeRepository::new(databento_config.clone()).await;
    let depth_result = exchange::DatabentoDepthRepository::new(databento_config).await;

    match (trade_result, depth_result) {
        (Ok(trade), Ok(depth)) => {
            let trade_repo = Arc::new(trade);
            let depth_repo = Arc::new(depth);
            let service = Arc::new(data::MarketDataService::with_download_repo(
                trade_repo.clone(),
                depth_repo.clone(),
                trade_repo.clone(),
            ));
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
}

/// Result of Rithmic service initialization
pub struct RithmicServiceResult {
    pub client: Arc<tokio::sync::Mutex<exchange::RithmicClient>>,
    pub trade_repo: Arc<exchange::RithmicTradeRepository>,
    pub depth_repo: Arc<exchange::RithmicDepthRepository>,
}

/// Initialize Rithmic services from a feed config and password
///
/// Creates a RithmicClient, connects to the specified environment,
/// and creates repository instances.
pub async fn initialize_rithmic_service(
    feed_config: &data::feed::RithmicFeedConfig,
    password: &str,
) -> Result<RithmicServiceResult, exchange::Error> {
    let (status_tx, _status_rx) = tokio::sync::mpsc::unbounded_channel();

    let (local_config, rithmic_config) =
        exchange::RithmicConfig::from_feed_config(feed_config, password)?;

    let mut client = exchange::RithmicClient::new(local_config, status_tx);
    client.connect(&rithmic_config).await?;

    let client = Arc::new(tokio::sync::Mutex::new(client));

    let trade_repo = Arc::new(exchange::RithmicTradeRepository::new(client.clone(), "CME"));
    let depth_repo = Arc::new(exchange::RithmicDepthRepository::new());

    log::info!("Rithmic service initialized successfully");

    Ok(RithmicServiceResult {
        client,
        trade_repo,
        depth_repo,
    })
}

/// Create replay engine for historical data playback
/// Returns None if market data result is not available
pub fn create_replay_engine(
    market_data_result: Option<&MarketDataServiceResult>,
) -> Option<Arc<tokio::sync::Mutex<data::services::ReplayEngine>>> {
    let result = market_data_result?;

    // Replay uses trades only - depth data is too large to load historically
    let config = data::services::ReplayEngineConfig {
        load_depth: false,
        ..Default::default()
    };

    Some(Arc::new(tokio::sync::Mutex::new(
        data::services::ReplayEngine::new(config, result.trade_repo.clone(), None),
    )))
}

/// Combined result of all service initialization
#[derive(Clone)]
pub struct AllServicesResult {
    pub market_data: Option<MarketDataServiceResult>,
    #[cfg(feature = "options")]
    pub options: Option<Arc<data::services::OptionsDataService>>,
}

impl std::fmt::Debug for AllServicesResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AllServicesResult")
            .field("market_data", &self.market_data.is_some())
            .finish()
    }
}

/// Initialize all services asynchronously — called via Task::perform at startup.
pub async fn initialize_all_services() -> AllServicesResult {
    let market_data = initialize_market_data_service().await;

    #[cfg(feature = "options")]
    let (options, _gex) = initialize_options_services().await;

    AllServicesResult {
        market_data,
        #[cfg(feature = "options")]
        options,
    }
}
