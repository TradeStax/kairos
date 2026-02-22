//! Databento HTTP client initialization and configuration

use super::{DatabentoConfig, DatabentoError};
use databento::HistoricalClient;

/// Initialize a Databento historical API client
pub fn create_historical_client(
    config: &DatabentoConfig,
) -> Result<HistoricalClient, DatabentoError> {
    let client = HistoricalClient::builder()
        .key(config.api_key.clone())?
        .build()?;

    log::info!("Databento HistoricalClient initialized");
    Ok(client)
}

/// Validate API key format — delegates to the shared implementation in `util`.
pub fn validate_api_key(key: &str) -> bool {
    crate::util::validate_api_key(key)
}
