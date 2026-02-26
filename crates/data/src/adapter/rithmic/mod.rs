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

/// Rithmic error types
#[derive(Debug, thiserror::Error)]
pub enum RithmicError {
    #[error("Rithmic connection error: {0}")]
    Connection(String),
    #[error("Rithmic authentication error: {0}")]
    Auth(String),
    #[error("Rithmic subscription error: {0}")]
    Subscription(String),
    #[error("Rithmic data error: {0}")]
    Data(String),
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

/// Rithmic configuration
#[derive(Debug, Clone)]
pub struct RithmicConfig {
    pub env: protocol::RithmicEnv,
    pub connect_strategy: protocol::ConnectStrategy,
    pub auto_reconnect: bool,
    pub cache_dir: std::path::PathBuf,
    /// Number of parallel history plant connections (default: 3).
    /// Set to `Some(1)` to disable parallel fetching.
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
            history_pool_size: None, // uses pool default (3)
        }
    }
}

impl RithmicConfig {
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

    pub fn from_connection_config(
        connection_config: &crate::connection::config::RithmicConnectionConfig,
        password: &str,
    ) -> Result<(Self, protocol::RithmicConnectionConfig), RithmicError> {
        if connection_config.user_id.trim().is_empty() {
            return Err(RithmicError::Config("User ID is required".to_string()));
        }
        if password.trim().is_empty() {
            return Err(RithmicError::Config("Password is required".to_string()));
        }
        if connection_config.system_name.trim().is_empty() {
            return Err(RithmicError::Config("System name is required".to_string()));
        }

        let env = protocol::RithmicEnv::Demo;
        let server_url = connection_config.server.url().to_string();
        let rithmic_config = protocol::RithmicConnectionConfig {
            env,
            user: connection_config.user_id.clone(),
            password: password.to_string(),
            system_name: connection_config.system_name.clone(),
            url: server_url.clone(),
            beta_url: server_url,
            account_id: connection_config
                .subscribed_tickers
                .first()
                .cloned()
                .unwrap_or_default(),
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

/// Build a `DataIndex` contribution for Rithmic realtime subscriptions.
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
            schema: "trades".to_string(),
        };
        index.add_contribution(key, feed_id, BTreeSet::new(), true);
    }
    index
}
