//! Exchange adapters for market data providers.
//!
//! Each adapter is behind a feature flag:
//! - [`databento`] (`feature = "databento"`) — CME Globex historical data via Databento API
//! - [`rithmic`] (`feature = "rithmic"`) — CME real-time streaming via Rithmic R|Protocol
//!
//! Adapters convert external wire formats into domain types from [`crate::domain`].

#[cfg(feature = "databento")]
pub mod databento;

#[cfg(feature = "rithmic")]
pub mod rithmic;

/// Validate API key format (non-empty, >= 10 chars)
pub(crate) fn validate_api_key(key: &str) -> bool {
    !key.is_empty() && key.len() >= 10
}
