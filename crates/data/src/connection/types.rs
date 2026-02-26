//! Connection types — provider, status, capability, and dataset info.

use super::config::{ConnectionConfig, DatabentoConnectionConfig, RithmicConnectionConfig};
use crate::domain::types::{DateRange, FeedId};
use serde::{Deserialize, Serialize};

/// Whether a connection is realtime (live) or historical (dataset)
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionKind {
    #[default]
    Realtime,
    Historical(HistoricalDatasetInfo),
}

/// Metadata about a downloaded historical dataset
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalDatasetInfo {
    pub ticker: String,
    pub date_range: DateRange,
    pub schema: String,
    pub trade_count: Option<usize>,
    pub file_size_bytes: Option<u64>,
}

/// Data connection provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionProvider {
    Databento,
    Rithmic,
}

impl ConnectionProvider {
    pub const ALL: [ConnectionProvider; 2] =
        [ConnectionProvider::Databento, ConnectionProvider::Rithmic];

    pub fn display_name(&self) -> &'static str {
        match self {
            ConnectionProvider::Databento => "Databento",
            ConnectionProvider::Rithmic => "Rithmic",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ConnectionProvider::Databento => "Historical trades, depth, OHLCV via Databento API",
            ConnectionProvider::Rithmic => {
                "Realtime + historical trades, depth, quotes via Rithmic"
            }
        }
    }

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

/// Status of a data connection
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
    pub fn is_connected(&self) -> bool {
        matches!(
            self,
            ConnectionStatus::Connected
                | ConnectionStatus::Downloading { .. }
                | ConnectionStatus::Loading { .. }
                | ConnectionStatus::Streaming { .. }
        )
    }

    pub fn is_error(&self) -> bool {
        matches!(self, ConnectionStatus::Error(_))
    }

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

/// Capabilities a connection can provide
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

/// A configured data connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: FeedId,
    pub name: String,
    pub provider: ConnectionProvider,
    #[serde(default)]
    pub kind: ConnectionKind,
    pub config: ConnectionConfig,
    pub enabled: bool,
    #[serde(skip)]
    pub status: ConnectionStatus,
    pub priority: u32,
    #[serde(default)]
    pub auto_connect: bool,
}

impl Connection {
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
            auto_connect: false,
        }
    }

    pub fn is_historical(&self) -> bool {
        matches!(self.kind, ConnectionKind::Historical(_))
    }

    pub fn is_realtime(&self) -> bool {
        matches!(self.kind, ConnectionKind::Realtime)
    }

    pub fn dataset_info(&self) -> Option<&HistoricalDatasetInfo> {
        match &self.kind {
            ConnectionKind::Historical(info) => Some(info),
            _ => None,
        }
    }

    pub fn capabilities(&self) -> Vec<ConnectionCapability> {
        self.provider.capabilities()
    }

    pub fn has_capability(&self, cap: ConnectionCapability) -> bool {
        self.capabilities().contains(&cap)
    }

    pub fn databento_config(&self) -> Option<&DatabentoConnectionConfig> {
        match &self.config {
            ConnectionConfig::Databento(cfg) => Some(cfg),
            _ => None,
        }
    }

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
