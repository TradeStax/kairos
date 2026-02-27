//! Databento HTTP client initialization.
//!
//! Provides factory functions for creating authenticated
//! [`HistoricalClient`] instances from a [`DatabentoConfig`].

use databento::HistoricalClient;

use super::{DatabentoConfig, DatabentoError};

/// Creates an authenticated [`HistoricalClient`] from the given config
pub fn create_historical_client(
    config: &DatabentoConfig,
) -> Result<HistoricalClient, DatabentoError> {
    let client = HistoricalClient::builder()
        .key(config.api_key.clone())?
        .build()?;
    log::info!("Databento HistoricalClient initialized");
    Ok(client)
}

/// Returns `true` if the API key passes basic format validation
#[must_use]
pub fn validate_api_key(key: &str) -> bool {
    crate::adapter::validate_api_key(key)
}
