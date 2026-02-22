//! Data Feed Types
//!
//! Core types for the data feed connection model.

use crate::DateRange;
use serde::{Deserialize, Serialize};

/// Unique identifier for a data feed (re-exported from domain layer)
pub use crate::domain::types::FeedId;

/// Whether a feed is realtime (live connection) or historical (downloaded dataset)
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeedKind {
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

/// Data feed provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FeedProvider {
    /// Databento API for historical CME futures data
    Databento,
    /// Rithmic for realtime + historical futures data
    Rithmic,
}

impl FeedProvider {
    /// Get all providers
    pub const ALL: [FeedProvider; 2] = [FeedProvider::Databento, FeedProvider::Rithmic];

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            FeedProvider::Databento => "Databento",
            FeedProvider::Rithmic => "Rithmic",
        }
    }

    /// Get description of what this provider offers
    pub fn description(&self) -> &'static str {
        match self {
            FeedProvider::Databento => "Historical trades, depth, OHLCV via Databento API",
            FeedProvider::Rithmic => "Realtime + historical trades, depth, quotes via Rithmic",
        }
    }

    /// Get capabilities this provider supports
    pub fn capabilities(&self) -> Vec<FeedCapability> {
        match self {
            FeedProvider::Databento => vec![
                FeedCapability::HistoricalTrades,
                FeedCapability::HistoricalDepth,
                FeedCapability::HistoricalOHLCV,
            ],
            FeedProvider::Rithmic => vec![
                FeedCapability::HistoricalTrades,
                FeedCapability::HistoricalDepth,
                FeedCapability::RealtimeTrades,
                FeedCapability::RealtimeDepth,
                FeedCapability::RealtimeQuotes,
            ],
        }
    }
}

impl std::fmt::Display for FeedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Status of a data feed connection
#[derive(Debug, Clone, Default, PartialEq)]
pub enum FeedStatus {
    /// Not connected
    #[default]
    Disconnected,
    /// Connection in progress
    Connecting,
    /// Successfully connected and ready
    Connected,
    /// Connection error
    Error(String),
    /// Downloading historical data
    Downloading {
        current_day: usize,
        total_days: usize,
    },
}

impl FeedStatus {
    pub fn is_connected(&self) -> bool {
        matches!(self, FeedStatus::Connected | FeedStatus::Downloading { .. })
    }

    pub fn is_error(&self) -> bool {
        matches!(self, FeedStatus::Error(_))
    }

    pub fn display_text(&self) -> String {
        match self {
            FeedStatus::Disconnected => "Disconnected".to_string(),
            FeedStatus::Connecting => "Connecting...".to_string(),
            FeedStatus::Connected => "Connected".to_string(),
            FeedStatus::Error(msg) => format!("Error: {}", msg),
            FeedStatus::Downloading {
                current_day,
                total_days,
            } => {
                format!("Downloading {}/{}", current_day, total_days)
            }
        }
    }
}

/// Capabilities a feed can provide
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FeedCapability {
    HistoricalTrades,
    HistoricalDepth,
    HistoricalOHLCV,
    RealtimeTrades,
    RealtimeDepth,
    RealtimeQuotes,
}

impl FeedCapability {
    pub fn display_name(&self) -> &'static str {
        match self {
            FeedCapability::HistoricalTrades => "Historical trades",
            FeedCapability::HistoricalDepth => "Historical depth",
            FeedCapability::HistoricalOHLCV => "Historical OHLCV",
            FeedCapability::RealtimeTrades => "Realtime trades",
            FeedCapability::RealtimeDepth => "Realtime depth",
            FeedCapability::RealtimeQuotes => "Realtime quotes",
        }
    }

    pub fn is_realtime(&self) -> bool {
        matches!(
            self,
            FeedCapability::RealtimeTrades
                | FeedCapability::RealtimeDepth
                | FeedCapability::RealtimeQuotes
        )
    }
}

impl std::fmt::Display for FeedCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Rithmic connection environment
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RithmicEnvironment {
    /// Demo/paper trading
    #[default]
    Demo,
    /// Live production
    Live,
    /// Test/sandbox
    Test,
}

impl RithmicEnvironment {
    pub const ALL: [RithmicEnvironment; 3] = [
        RithmicEnvironment::Demo,
        RithmicEnvironment::Live,
        RithmicEnvironment::Test,
    ];
}

impl std::fmt::Display for RithmicEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RithmicEnvironment::Demo => write!(f, "Demo"),
            RithmicEnvironment::Live => write!(f, "Live"),
            RithmicEnvironment::Test => write!(f, "Test"),
        }
    }
}

/// Provider-specific feed configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeedConfig {
    Databento(DatabentoFeedConfig),
    Rithmic(RithmicFeedConfig),
}

/// Databento-specific feed configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatabentoFeedConfig {
    /// Which data schemas to enable
    pub enabled_schemas: Vec<String>,
    /// Enable local caching
    pub cache_enabled: bool,
    /// Maximum days to keep in cache
    pub cache_max_days: u32,
}

impl Default for DatabentoFeedConfig {
    fn default() -> Self {
        Self {
            enabled_schemas: vec![
                "trades".to_string(),
                "mbp10".to_string(),
                "ohlcv1m".to_string(),
            ],
            cache_enabled: true,
            cache_max_days: 90,
        }
    }
}

/// Rithmic-specific feed configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RithmicFeedConfig {
    /// Connection environment
    pub environment: RithmicEnvironment,
    /// System name for Rithmic connection
    pub system_name: String,
    /// User ID for authentication
    pub user_id: String,
    /// Auto-reconnect on disconnect
    pub auto_reconnect: bool,
    /// Tickers to subscribe to for realtime data
    pub subscribed_tickers: Vec<String>,
}

impl Default for RithmicFeedConfig {
    fn default() -> Self {
        Self {
            environment: RithmicEnvironment::Demo,
            system_name: String::new(),
            user_id: String::new(),
            auto_reconnect: true,
            subscribed_tickers: vec![],
        }
    }
}

/// A configured data feed connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFeed {
    /// Unique identifier
    pub id: FeedId,
    /// User-facing name
    pub name: String,
    /// Provider type
    pub provider: FeedProvider,
    /// Realtime connection or historical dataset
    #[serde(default)]
    pub kind: FeedKind,
    /// Provider-specific configuration
    pub config: FeedConfig,
    /// Whether this feed is enabled
    pub enabled: bool,
    /// Connection status (not persisted)
    #[serde(skip)]
    pub status: FeedStatus,
    /// Priority for data resolution (lower = higher priority)
    pub priority: u32,
    /// Automatically connect this feed on application startup
    #[serde(default)]
    pub auto_connect: bool,
}

impl DataFeed {
    /// Create a new Databento feed with defaults
    pub fn new_databento(name: impl Into<String>) -> Self {
        Self {
            id: FeedId::new_v4(),
            name: name.into(),
            provider: FeedProvider::Databento,
            kind: FeedKind::Realtime,
            config: FeedConfig::Databento(DatabentoFeedConfig::default()),
            enabled: true,
            status: FeedStatus::Disconnected,
            priority: 10,
            auto_connect: false,
        }
    }

    /// Create a new Rithmic feed with defaults
    pub fn new_rithmic(name: impl Into<String>) -> Self {
        Self {
            id: FeedId::new_v4(),
            name: name.into(),
            provider: FeedProvider::Rithmic,
            kind: FeedKind::Realtime,
            config: FeedConfig::Rithmic(RithmicFeedConfig::default()),
            enabled: true,
            status: FeedStatus::Disconnected,
            priority: 5,
            auto_connect: false,
        }
    }

    /// Create a new historical dataset feed (Databento)
    pub fn new_historical_databento(name: impl Into<String>, info: HistoricalDatasetInfo) -> Self {
        Self {
            id: FeedId::new_v4(),
            name: name.into(),
            provider: FeedProvider::Databento,
            kind: FeedKind::Historical(info),
            config: FeedConfig::Databento(DatabentoFeedConfig::default()),
            enabled: true,
            status: FeedStatus::Disconnected,
            priority: 100,
            auto_connect: false,
        }
    }

    /// Is this a historical dataset?
    pub fn is_historical(&self) -> bool {
        matches!(self.kind, FeedKind::Historical(_))
    }

    /// Is this a realtime connection?
    pub fn is_realtime(&self) -> bool {
        matches!(self.kind, FeedKind::Realtime)
    }

    /// Get the dataset info if this is a historical feed
    pub fn dataset_info(&self) -> Option<&HistoricalDatasetInfo> {
        match &self.kind {
            FeedKind::Historical(info) => Some(info),
            _ => None,
        }
    }

    /// Get capabilities for this feed
    pub fn capabilities(&self) -> Vec<FeedCapability> {
        self.provider.capabilities()
    }

    /// Check if this feed supports a given capability
    pub fn has_capability(&self, cap: FeedCapability) -> bool {
        self.capabilities().contains(&cap)
    }

    /// Get the Databento config, if this is a Databento feed
    pub fn databento_config(&self) -> Option<&DatabentoFeedConfig> {
        match &self.config {
            FeedConfig::Databento(cfg) => Some(cfg),
            _ => None,
        }
    }

    /// Get the Rithmic config, if this is a Rithmic feed
    pub fn rithmic_config(&self) -> Option<&RithmicFeedConfig> {
        match &self.config {
            FeedConfig::Rithmic(cfg) => Some(cfg),
            _ => None,
        }
    }
}

impl PartialEq for DataFeed {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feed_provider_capabilities() {
        let databento_caps = FeedProvider::Databento.capabilities();
        assert!(databento_caps.contains(&FeedCapability::HistoricalTrades));
        assert!(!databento_caps.contains(&FeedCapability::RealtimeTrades));

        let rithmic_caps = FeedProvider::Rithmic.capabilities();
        assert!(rithmic_caps.contains(&FeedCapability::RealtimeTrades));
        assert!(rithmic_caps.contains(&FeedCapability::HistoricalTrades));
    }

    #[test]
    fn test_data_feed_creation() {
        let feed = DataFeed::new_databento("My Databento");
        assert_eq!(feed.provider, FeedProvider::Databento);
        assert!(feed.enabled);
        assert_eq!(feed.priority, 10);
        assert!(feed.databento_config().is_some());
        assert!(feed.rithmic_config().is_none());

        let feed = DataFeed::new_rithmic("My Rithmic");
        assert_eq!(feed.provider, FeedProvider::Rithmic);
        assert_eq!(feed.priority, 5);
        assert!(feed.rithmic_config().is_some());
    }

    #[test]
    fn test_feed_status() {
        assert!(!FeedStatus::Disconnected.is_connected());
        assert!(FeedStatus::Connected.is_connected());
        assert!(
            FeedStatus::Downloading {
                current_day: 1,
                total_days: 5
            }
            .is_connected()
        );
        assert!(FeedStatus::Error("test".to_string()).is_error());
    }

    #[test]
    fn test_feed_serialization() {
        let feed = DataFeed::new_databento("Test Feed");
        let json = serde_json::to_string(&feed).unwrap();
        let deserialized: DataFeed = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, feed.id);
        assert_eq!(deserialized.name, "Test Feed");
        assert_eq!(deserialized.status, FeedStatus::Disconnected); // skip
    }

    #[test]
    fn test_capability_display() {
        assert_eq!(
            FeedCapability::HistoricalTrades.display_name(),
            "Historical trades"
        );
        assert!(!FeedCapability::HistoricalTrades.is_realtime());
        assert!(FeedCapability::RealtimeTrades.is_realtime());
    }
}
