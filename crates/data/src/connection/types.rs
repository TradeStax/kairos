//! Connection types — provider, status, capability, and dataset info.
//!
//! Defines the core abstractions for data feed connections: what provider
//! they use, what they can do, and their current lifecycle state.

use super::config::{ConnectionConfig, DatabentoConnectionConfig, RithmicConnectionConfig};
use crate::domain::types::{DateRange, FeedId};
use serde::{Deserialize, Serialize};

/// Whether a connection provides real-time streaming or historical dataset access.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionKind {
    /// Live streaming connection
    #[default]
    Realtime,
    /// Downloaded historical dataset
    Historical(HistoricalDatasetInfo),
}

/// Metadata about a downloaded historical dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalDatasetInfo {
    /// Ticker symbol (e.g. "ES.c.0")
    pub ticker: String,
    /// Date range covered by the dataset
    pub date_range: DateRange,
    /// Schema type (e.g. "trades", "depth")
    pub schema: String,
    /// Number of trade records, if known
    pub trade_count: Option<usize>,
    /// Total file size in bytes, if known
    pub file_size_bytes: Option<u64>,
}

/// Data connection provider identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionProvider {
    /// Databento — historical CME Globex data via API
    Databento,
    /// Rithmic — real-time and historical CME data via R|Protocol
    Rithmic,
}

impl ConnectionProvider {
    /// All supported providers.
    pub const ALL: [ConnectionProvider; 2] =
        [ConnectionProvider::Databento, ConnectionProvider::Rithmic];

    /// Returns a human-readable name for this provider
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            ConnectionProvider::Databento => "Databento",
            ConnectionProvider::Rithmic => "Rithmic",
        }
    }

    /// Returns a short description of the provider's capabilities
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            ConnectionProvider::Databento => "Historical trades, depth, OHLCV via Databento API",
            ConnectionProvider::Rithmic => {
                "Realtime + historical trades, depth, quotes via Rithmic"
            }
        }
    }

    /// Returns the list of capabilities this provider supports
    #[must_use]
    pub fn capabilities(&self) -> Vec<ConnectionCapability> {
        match self {
            ConnectionProvider::Databento => vec![
                ConnectionCapability::HistoricalTrades,
                ConnectionCapability::HistoricalDepth,
                ConnectionCapability::HistoricalOHLCV,
            ],
            ConnectionProvider::Rithmic => vec![
                ConnectionCapability::HistoricalTrades,
                ConnectionCapability::HistoricalDepth,
                ConnectionCapability::RealtimeTrades,
                ConnectionCapability::RealtimeDepth,
                ConnectionCapability::RealtimeQuotes,
            ],
        }
    }
}

impl std::fmt::Display for ConnectionProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Lifecycle status of a data connection.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Loading {
        ticker: String,
        days_complete: usize,
        days_total: usize,
    },
    Streaming {
        subscriptions: usize,
        events_received: u64,
    },
    Reconnecting {
        attempt: u32,
    },
    Error(String),
    Downloading {
        current_day: usize,
        total_days: usize,
    },
}

impl ConnectionStatus {
    /// Returns `true` if the connection is in an active/usable state
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(
            self,
            ConnectionStatus::Connected
                | ConnectionStatus::Downloading { .. }
                | ConnectionStatus::Loading { .. }
                | ConnectionStatus::Streaming { .. }
        )
    }

    /// Returns `true` if the connection is in an error state
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, ConnectionStatus::Error(_))
    }

    /// Returns a human-readable status string for UI display
    #[must_use]
    pub fn display_text(&self) -> String {
        match self {
            ConnectionStatus::Disconnected => "Disconnected".to_string(),
            ConnectionStatus::Connecting => "Connecting...".to_string(),
            ConnectionStatus::Connected => "Connected".to_string(),
            ConnectionStatus::Loading {
                ticker,
                days_complete,
                days_total,
            } => format!("Loading {} ({}/{})", ticker, days_complete, days_total),
            ConnectionStatus::Streaming {
                subscriptions,
                events_received,
            } => format!(
                "Streaming ({} subs, {} events)",
                subscriptions, events_received
            ),
            ConnectionStatus::Reconnecting { attempt } => {
                format!("Reconnecting (attempt {})", attempt)
            }
            ConnectionStatus::Error(msg) => format!("Error: {}", msg),
            ConnectionStatus::Downloading {
                current_day,
                total_days,
            } => format!("Downloading {}/{}", current_day, total_days),
        }
    }
}

/// A specific capability a connection can provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionCapability {
    HistoricalTrades,
    HistoricalDepth,
    HistoricalOHLCV,
    RealtimeTrades,
    RealtimeDepth,
    RealtimeQuotes,
}

impl ConnectionCapability {
    /// Returns a human-readable label for this capability
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            ConnectionCapability::HistoricalTrades => "Historical trades",
            ConnectionCapability::HistoricalDepth => "Historical depth",
            ConnectionCapability::HistoricalOHLCV => "Historical OHLCV",
            ConnectionCapability::RealtimeTrades => "Realtime trades",
            ConnectionCapability::RealtimeDepth => "Realtime depth",
            ConnectionCapability::RealtimeQuotes => "Realtime quotes",
        }
    }

    /// Returns `true` if this is a real-time (streaming) capability
    #[must_use]
    pub fn is_realtime(&self) -> bool {
        matches!(
            self,
            ConnectionCapability::RealtimeTrades
                | ConnectionCapability::RealtimeDepth
                | ConnectionCapability::RealtimeQuotes
        )
    }
}

impl std::fmt::Display for ConnectionCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// A configured data connection with provider, config, and runtime status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Unique identifier for this connection
    pub id: FeedId,
    /// User-facing name
    pub name: String,
    /// Data provider (Databento or Rithmic)
    pub provider: ConnectionProvider,
    /// Whether this is a live or historical connection
    #[serde(default)]
    pub kind: ConnectionKind,
    /// Provider-specific configuration
    pub config: ConnectionConfig,
    /// Whether this connection is enabled
    pub enabled: bool,
    /// Current lifecycle status (transient, not persisted)
    #[serde(skip)]
    pub status: ConnectionStatus,
    /// Priority for connection resolution (lower = preferred)
    pub priority: u32,
    /// Whether to connect automatically on startup
    #[serde(default)]
    pub auto_connect: bool,
}

impl Connection {
    /// Creates a new Databento connection with default configuration
    #[must_use]
    pub fn new_databento(name: impl Into<String>) -> Self {
        Self {
            id: FeedId::new_v4(),
            name: name.into(),
            provider: ConnectionProvider::Databento,
            kind: ConnectionKind::Realtime,
            config: ConnectionConfig::Databento(DatabentoConnectionConfig::default()),
            enabled: true,
            status: ConnectionStatus::Disconnected,
            priority: 10,
            auto_connect: false,
        }
    }

    /// Creates a new Rithmic connection with default configuration
    #[must_use]
    pub fn new_rithmic(name: impl Into<String>) -> Self {
        Self {
            id: FeedId::new_v4(),
            name: name.into(),
            provider: ConnectionProvider::Rithmic,
            kind: ConnectionKind::Realtime,
            config: ConnectionConfig::Rithmic(RithmicConnectionConfig::default()),
            enabled: true,
            status: ConnectionStatus::Disconnected,
            priority: 5,
            auto_connect: false,
        }
    }

    /// Creates a new historical Databento connection for a downloaded dataset.
    /// Auto-connect is enabled so tickers appear on startup.
    #[must_use]
    pub fn new_historical_databento(name: impl Into<String>, info: HistoricalDatasetInfo) -> Self {
        Self {
            id: FeedId::new_v4(),
            name: name.into(),
            provider: ConnectionProvider::Databento,
            kind: ConnectionKind::Historical(info),
            config: ConnectionConfig::Databento(DatabentoConnectionConfig::default()),
            enabled: true,
            status: ConnectionStatus::Disconnected,
            priority: 100,
            auto_connect: true,
        }
    }

    /// Returns `true` if this is a historical dataset connection
    #[must_use]
    pub fn is_historical(&self) -> bool {
        matches!(self.kind, ConnectionKind::Historical(_))
    }

    /// Returns `true` if this is a real-time streaming connection
    #[must_use]
    pub fn is_realtime(&self) -> bool {
        matches!(self.kind, ConnectionKind::Realtime)
    }

    /// Returns the historical dataset info, if this is a historical connection
    #[must_use]
    pub fn dataset_info(&self) -> Option<&HistoricalDatasetInfo> {
        match &self.kind {
            ConnectionKind::Historical(info) => Some(info),
            _ => None,
        }
    }

    /// Returns the capabilities inherited from the provider
    #[must_use]
    pub fn capabilities(&self) -> Vec<ConnectionCapability> {
        self.provider.capabilities()
    }

    /// Returns `true` if this connection supports a specific capability
    #[must_use]
    pub fn has_capability(&self, cap: ConnectionCapability) -> bool {
        self.capabilities().contains(&cap)
    }

    /// Returns the Databento-specific config, if this is a Databento connection
    #[must_use]
    pub fn databento_config(&self) -> Option<&DatabentoConnectionConfig> {
        match &self.config {
            ConnectionConfig::Databento(cfg) => Some(cfg),
            _ => None,
        }
    }

    /// Returns the Rithmic-specific config, if this is a Rithmic connection
    #[must_use]
    pub fn rithmic_config(&self) -> Option<&RithmicConnectionConfig> {
        match &self.config {
            ConnectionConfig::Rithmic(cfg) => Some(cfg),
            _ => None,
        }
    }
}

impl PartialEq for Connection {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_creation() {
        let conn = Connection::new_databento("My Databento");
        assert_eq!(conn.provider, ConnectionProvider::Databento);
        assert!(conn.enabled);
        assert_eq!(conn.priority, 10);
        assert!(conn.databento_config().is_some());
        assert!(conn.rithmic_config().is_none());
    }

    #[test]
    fn test_connection_status() {
        assert!(!ConnectionStatus::Disconnected.is_connected());
        assert!(ConnectionStatus::Connected.is_connected());
        assert!(ConnectionStatus::Error("test".to_string()).is_error());
    }

    #[test]
    fn test_provider_capabilities() {
        let databento_caps = ConnectionProvider::Databento.capabilities();
        assert!(databento_caps.contains(&ConnectionCapability::HistoricalTrades));
        assert!(!databento_caps.contains(&ConnectionCapability::RealtimeTrades));

        let rithmic_caps = ConnectionProvider::Rithmic.capabilities();
        assert!(rithmic_caps.contains(&ConnectionCapability::RealtimeTrades));
    }
}
