use std::sync::Arc;

/// Initialize options services from environment
pub fn initialize_options_services() -> (
    Option<Arc<data::services::OptionsDataService>>,
    Arc<data::services::GexCalculationService>,
) {
    // GEX service is always available (no I/O, pure computation)
    let gex_service = Arc::new(data::services::GexCalculationService::new());

    // Try to initialize Massive options service from environment
    let options_service = match std::env::var("MASSIVE_API_KEY") {
        Ok(api_key) if !api_key.is_empty() => {
            log::info!("MASSIVE_API_KEY found, initializing options data service");

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
        _ => {
            log::info!("MASSIVE_API_KEY not set - options data features disabled");
            log::info!("To enable: export MASSIVE_API_KEY=your_polygon_api_key");
            None
        }
    };

    (options_service, gex_service)
}

/// Initialize market data repositories and service
pub fn initialize_market_data_service() -> (
    Arc<data::MarketDataService>,
    Arc<exchange::DatabentoTradeRepository>,
    Arc<exchange::DatabentoDepthRepository>,
) {
    // Initialize Databento configuration
    let databento_config = match exchange::adapter::databento::DatabentoConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            log::warn!("Failed to load Databento config from environment: {}, using defaults", e);
            exchange::adapter::databento::DatabentoConfig::default()
        }
    };

    // Create runtime for async repository initialization
    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");

    // Create repository instances (async)
    let (trade_repo, depth_repo) = rt.block_on(async {
        let trade = exchange::DatabentoTradeRepository::new(databento_config.clone())
            .await
            .expect("Failed to create trade repository");
        let depth = exchange::DatabentoDepthRepository::new(databento_config)
            .await
            .expect("Failed to create depth repository");
        (Arc::new(trade), Arc::new(depth))
    });

    // Create market data service
    let market_data_service = Arc::new(
        data::MarketDataService::new(trade_repo.clone(), depth_repo.clone())
    );

    (market_data_service, trade_repo, depth_repo)
}

/// Create replay engine for historical data playback
pub fn create_replay_engine(
    trade_repo: Arc<exchange::DatabentoTradeRepository>,
    depth_repo: Option<Arc<exchange::DatabentoDepthRepository>>,
) -> Option<Arc<std::sync::Mutex<data::services::ReplayEngine>>> {
    // Convert concrete types to trait objects
    let depth_repo_dyn: Option<Arc<dyn data::DepthRepository + Send + Sync>> =
        depth_repo.map(|r| r as Arc<dyn data::DepthRepository + Send + Sync>);

    Some(Arc::new(std::sync::Mutex::new(
        data::services::ReplayEngine::with_default_config(trade_repo, depth_repo_dyn)
    )))
}
