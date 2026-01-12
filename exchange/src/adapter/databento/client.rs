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

/// Validate API key format
pub fn validate_api_key(key: &str) -> bool {
    !key.is_empty() && key.len() > 10
}
