//! Rithmic adapter for CME futures real-time market data.
//!
//! Feature-gated: `feature = "rithmic"`.
//!
//! - `RithmicClient` — connects to Rithmic, subscribes to tickers, streams trades and depth
//! - `RithmicConfig` — environment, reconnect policy, cache directory
//! - [`protocol`] — R|Protocol WebSocket message encoding, request/response, ping keepalive
//! - `plants` — ticker plant (live data) and history plant (historical replay)
//! - [`streaming`] — `RithmicStream` for consuming live market events
//! - [`mapper`] — Rithmic wire types to domain `Trade` and `Depth`

pub mod client;
pub mod mapper;
pub(crate) mod plants;
pub mod pool;
pub mod protocol;
pub mod streaming;

pub use client::RithmicClient;
pub use pool::HistoryPlantPool;
pub use streaming::RithmicStream;

/// Errors originating from the Rithmic adapter layer.
#[derive(Debug, thiserror::Error)]
pub enum RithmicError {
    /// WebSocket or network-level connection failure
    #[error("Rithmic connection error: {0}")]
    Connection(String),
    /// Login or credential rejection
    #[error("Rithmic authentication error: {0}")]
    Auth(String),
    /// Market data subscription failure
    #[error("Rithmic subscription error: {0}")]
    Subscription(String),
    /// Data retrieval or decoding failure
    #[error("Rithmic data error: {0}")]
    Data(String),
    /// Invalid or missing configuration
    #[error("Rithmic configuration error: {0}")]
    Config(String),
}

impl From<RithmicError> for crate::Error {
    fn from(e: RithmicError) -> Self {
        match e {
            RithmicError::Connection(s) => crate::Error::Connection(s),
            RithmicError::Auth(s) => crate::Error::Config(s),
            RithmicError::Subscription(s) => crate::Error::Fetch(s),
            RithmicError::Data(s) => crate::Error::Fetch(s),
            RithmicError::Config(s) => crate::Error::Config(s),
        }
    }
}

/// High-level Rithmic adapter configuration.
///
/// Controls environment selection, reconnect behavior, cache location,
/// and history plant pool sizing.
#[derive(Debug, Clone)]
pub struct RithmicConfig {
    /// Target Rithmic environment (Demo, Live, Test)
    pub env: protocol::RithmicEnv,
    /// WebSocket connection strategy (simple, retry, alternate)
    pub connect_strategy: protocol::ConnectStrategy,
    /// Whether to automatically reconnect on disconnect.
    ///
    /// Currently stored for configuration purposes but not acted upon
    /// by the plant actors (they exit on disconnect). A future
    /// reconnect loop would consult this flag.
    pub auto_reconnect: bool,
    /// Directory for local Rithmic data cache
    pub cache_dir: std::path::PathBuf,
    /// Number of parallel history plant connections.
    /// Defaults to pool default (1) when `None`.
    pub history_pool_size: Option<usize>,
}

impl Default for RithmicConfig {
    fn default() -> Self {
        let cache_dir = std::path::PathBuf::from(".").join("kairos").join("rithmic");

        Self {
            env: protocol::RithmicEnv::Demo,
            connect_strategy: protocol::ConnectStrategy::Retry,
            auto_reconnect: true,
            cache_dir,
            history_pool_size: None, // uses pool default (1)
        }
    }
}

impl RithmicConfig {
    /// Creates a config pair by loading credentials from environment variables.
    ///
    /// Returns both the adapter config and the low-level connection config
    /// needed to authenticate with the Rithmic server.
    pub fn from_env(
        env: protocol::RithmicEnv,
    ) -> Result<(Self, protocol::RithmicConnectionConfig), RithmicError> {
        let rithmic_config = protocol::RithmicConnectionConfig::from_env(env)
            .map_err(|e| RithmicError::Config(format!("Failed to load Rithmic config: {}", e)))?;

        Ok((
            Self {
                env,
                ..Default::default()
            },
            rithmic_config,
        ))
    }

    /// Creates a config pair from an application-level connection config
    /// and a plaintext password.
    ///
    /// Validates that user ID, password, and system name are non-empty
    /// before constructing the protocol-level config. The server URL is
    /// resolved via the [`ServerResolver`] rather than being hardcoded.
    pub fn from_connection_config(
        connection_config: &crate::connection::config::RithmicConnectionConfig,
        password: &str,
        server_resolver: &crate::connection::ServerResolver,
    ) -> Result<(Self, protocol::RithmicConnectionConfig), RithmicError> {
        if connection_config.user_id.trim().is_empty() {
            return Err(RithmicError::Config("User ID is required".to_owned()));
        }
        if password.trim().is_empty() {
            return Err(RithmicError::Config("Password is required".to_owned()));
        }
        if connection_config.system_name.trim().is_empty() {
            return Err(RithmicError::Config("System name is required".to_owned()));
        }

        let env = match connection_config.environment {
            crate::connection::config::RithmicEnvironment::Demo => protocol::RithmicEnv::Demo,
            crate::connection::config::RithmicEnvironment::Live => protocol::RithmicEnv::Live,
            crate::connection::config::RithmicEnvironment::Test => protocol::RithmicEnv::Test,
        };
        let server_url = server_resolver
            .resolve(connection_config.server)
            .map_err(|e| RithmicError::Config(e.to_string()))?;
        let rithmic_config = protocol::RithmicConnectionConfig {
            env,
            user: connection_config.user_id.clone(),
            password: password.to_string(),
            system_name: connection_config.system_name.clone(),
            url: server_url.clone(),
            beta_url: server_url,
            // account_id is not used for market data connections
            // (ticker + history plants). It would be needed for an
            // order plant, which is not yet implemented.
            account_id: String::new(),
            fcm_id: String::new(),
            ib_id: String::new(),
        };

        let local = Self {
            env,
            auto_reconnect: connection_config.auto_reconnect,
            ..Default::default()
        };

        Ok((local, rithmic_config))
    }
}

/// Builds a [`DataIndex`](crate::domain::index::DataIndex) contribution
/// for Rithmic real-time trade subscriptions.
pub fn build_rithmic_contribution(
    feed_id: crate::domain::types::FeedId,
    subscribed_tickers: &[String],
) -> crate::domain::index::DataIndex {
    use crate::domain::index::{DataIndex, DataKey};
    use std::collections::BTreeSet;

    let mut index = DataIndex::new();
    for ticker in subscribed_tickers {
        let key = DataKey {
            ticker: ticker.clone(),
            schema: "trades".to_owned(),
        };
        index.add_contribution(key, feed_id, BTreeSet::new(), true);
    }
    index
}
