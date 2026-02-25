//! Rithmic adapter for CME futures market data
//!
//! This module provides integration with Rithmic's R | Protocol API for:
//! - Real-time streaming market data (trades, BBO, depth)
//! - Historical tick data retrieval
//! - Symbol search and front-month contract resolution

pub mod client;
pub mod mapper;
mod plants;
mod protocol;
pub mod streaming;

use super::AdapterError;

// Re-export main types
pub use client::RithmicClient;
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

use kairos_data::domain::error::{AppError, ErrorSeverity};

impl AppError for RithmicError {
    fn user_message(&self) -> String {
        match self {
            RithmicError::Connection(s) => {
                format!("Connection error: {}", s)
            }
            RithmicError::Auth(s) => format!("Authentication error: {}", s),
            RithmicError::Subscription(s) => {
                format!("Subscription error: {}", s)
            }
            RithmicError::Data(s) => format!("Data error: {}", s),
            RithmicError::Config(s) => format!("Configuration error: {}", s),
        }
    }

    fn is_retriable(&self) -> bool {
        matches!(
            self,
            RithmicError::Connection(_) | RithmicError::Subscription(_)
        )
    }

    fn severity(&self) -> ErrorSeverity {
        match self {
            RithmicError::Connection(_) | RithmicError::Subscription(_) => {
                ErrorSeverity::Recoverable
            }
            RithmicError::Auth(_) | RithmicError::Config(_) => ErrorSeverity::Critical,
            RithmicError::Data(_) => ErrorSeverity::Warning,
        }
    }
}

impl From<RithmicError> for AdapterError {
    fn from(err: RithmicError) -> Self {
        match err {
            RithmicError::Connection(s) => AdapterError::ConnectionError(s),
            RithmicError::Auth(s) => {
                AdapterError::InvalidRequest(format!("Rithmic auth: {}", s))
            }
            RithmicError::Subscription(s) => {
                AdapterError::InvalidRequest(format!("Rithmic sub: {}", s))
            }
            RithmicError::Data(s) => {
                AdapterError::ParseError(format!("Rithmic data: {}", s))
            }
            RithmicError::Config(s) => {
                AdapterError::InvalidRequest(format!("Rithmic config: {}", s))
            }
        }
    }
}

/// Rithmic configuration
#[derive(Debug, Clone)]
pub struct RithmicConfig {
    /// Rithmic environment (Demo, Live, etc.)
    pub env: protocol::RithmicEnv,
    /// Connection strategy
    pub connect_strategy: protocol::ConnectStrategy,
    /// Auto-reconnect on disconnection
    pub auto_reconnect: bool,
    /// Cache directory for historical data
    pub cache_dir: std::path::PathBuf,
}

impl Default for RithmicConfig {
    fn default() -> Self {
        let cache_dir = dirs_next::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("kairos")
            .join("rithmic");

        Self {
            env: protocol::RithmicEnv::Demo,
            connect_strategy: protocol::ConnectStrategy::Retry,
            auto_reconnect: true,
            cache_dir,
        }
    }
}

impl RithmicConfig {
    /// Create from environment variables for a given Rithmic environment.
    ///
    /// Returns both the local config and the upstream connection config
    /// needed to connect.
    pub fn from_env(
        env: protocol::RithmicEnv,
    ) -> Result<(Self, protocol::RithmicConnectionConfig), AdapterError> {
        let rithmic_config =
            protocol::RithmicConnectionConfig::from_env(env).map_err(|e| {
                AdapterError::InvalidRequest(format!(
                    "Failed to load Rithmic config from env: {}",
                    e
                ))
            })?;

        Ok((
            Self {
                env,
                ..Default::default()
            },
            rithmic_config,
        ))
    }

    /// Create from a UI feed config and password.
    ///
    /// Builds the connection config entirely from UI-provided
    /// fields — no environment variables required.
    pub fn from_feed_config(
        feed_config: &kairos_data::feed::RithmicFeedConfig,
        password: &str,
    ) -> Result<(Self, protocol::RithmicConnectionConfig), AdapterError> {
        // Validate required fields
        if feed_config.user_id.trim().is_empty() {
            return Err(AdapterError::InvalidRequest(
                "User ID is required".to_string(),
            ));
        }
        if password.trim().is_empty() {
            return Err(AdapterError::InvalidRequest(
                "Password is required".to_string(),
            ));
        }
        if feed_config.system_name.trim().is_empty() {
            return Err(AdapterError::InvalidRequest(
                "System name is required".to_string(),
            ));
        }

        let env = match feed_config.environment {
            kairos_data::feed::RithmicEnvironment::Demo => {
                protocol::RithmicEnv::Demo
            }
            kairos_data::feed::RithmicEnvironment::Live => {
                protocol::RithmicEnv::Live
            }
            kairos_data::feed::RithmicEnvironment::Test => {
                protocol::RithmicEnv::Test
            }
        };

        let server_url = feed_config.server.url().to_string();
        let rithmic_config = protocol::RithmicConnectionConfig {
            env,
            user: feed_config.user_id.clone(),
            password: password.to_string(),
            system_name: feed_config.system_name.clone(),
            url: server_url.clone(),
            beta_url: server_url,
            account_id: feed_config.account_id.clone(),
            fcm_id: String::new(),
            ib_id: String::new(),
        };

        let local = Self {
            env,
            auto_reconnect: feed_config.auto_reconnect,
            ..Default::default()
        };

        Ok((local, rithmic_config))
    }
}
