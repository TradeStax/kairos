//! Databento HTTP client initialization

use super::{DatabentoConfig, DatabentoError};
use databento::HistoricalClient;

pub fn create_historical_client(
    config: &DatabentoConfig,
) -> Result<HistoricalClient, DatabentoError> {
    let client = HistoricalClient::builder()
        .key(config.api_key.clone())?
        .build()?;
    log::info!("Databento HistoricalClient initialized");
    Ok(client)
}

pub fn validate_api_key(key: &str) -> bool {
    crate::adapter::validate_api_key(key)
}
