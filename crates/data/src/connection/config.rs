//! Connection configuration types

use serde::{Deserialize, Serialize};

/// Known Rithmic R|Protocol servers
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RithmicServer {
    #[default]
    Chicago,
    NewYork,
    Sydney,
    SaoPaolo,
    Colo75,
    Frankfurt,
    HongKong,
    Ireland,
    Mumbai,
    Seoul,
    CapeTown,
    Tokyo,
    Singapore,
}

impl RithmicServer {
    pub const ALL: [RithmicServer; 13] = [
        RithmicServer::Chicago,
        RithmicServer::NewYork,
        RithmicServer::Sydney,
        RithmicServer::SaoPaolo,
        RithmicServer::Colo75,
        RithmicServer::Frankfurt,
        RithmicServer::HongKong,
        RithmicServer::Ireland,
        RithmicServer::Mumbai,
        RithmicServer::Seoul,
        RithmicServer::CapeTown,
        RithmicServer::Tokyo,
        RithmicServer::Singapore,
    ];

    pub fn url(&self) -> &'static str {
        match self {
            RithmicServer::Chicago => "wss://rprotocol.rithmic.com:443",
            RithmicServer::NewYork => "wss://rprotocol-nyc.rithmic.com:443",
            RithmicServer::Sydney => "wss://rprotocol-au.rithmic.com:443",
            RithmicServer::SaoPaolo => "wss://rprotocol-br.rithmic.com:443",
            RithmicServer::Colo75 => "wss://rprotocol-colo75.rithmic.com:443",
            RithmicServer::Frankfurt => "wss://rprotocol-de.rithmic.com:443",
            RithmicServer::HongKong => "wss://rprotocol-hk.rithmic.com:443",
            RithmicServer::Ireland => "wss://rprotocol-ie.rithmic.com:443",
            RithmicServer::Mumbai => "wss://rprotocol-in.rithmic.com:443",
            RithmicServer::Seoul => "wss://rprotocol-kr.rithmic.com:443",
            RithmicServer::CapeTown => "wss://rprotocol-za.rithmic.com:443",
            RithmicServer::Tokyo => "wss://rprotocol-jp.rithmic.com:443",
            RithmicServer::Singapore => "wss://rprotocol-sg.rithmic.com:443",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            RithmicServer::Chicago => "Core (Chicago)",
            RithmicServer::NewYork => "New York",
            RithmicServer::Sydney => "Sydney",
            RithmicServer::SaoPaolo => "Sao Paolo",
            RithmicServer::Colo75 => "Colo75",
            RithmicServer::Frankfurt => "Frankfurt",
            RithmicServer::HongKong => "Hong Kong",
            RithmicServer::Ireland => "Ireland",
            RithmicServer::Mumbai => "Mumbai",
            RithmicServer::Seoul => "Seoul",
            RithmicServer::CapeTown => "Cape Town",
            RithmicServer::Tokyo => "Tokyo",
            RithmicServer::Singapore => "Singapore",
        }
    }

    pub fn from_url(url: &str) -> Option<RithmicServer> {
        RithmicServer::ALL.iter().find(|s| s.url() == url).copied()
    }
}

impl std::fmt::Display for RithmicServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Rithmic connection environment
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RithmicEnvironment {
    #[default]
    Demo,
    Live,
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

/// Provider-specific connection configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionConfig {
    Databento(DatabentoConnectionConfig),
    Rithmic(RithmicConnectionConfig),
}

/// Databento-specific connection configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatabentoConnectionConfig {
    pub enabled_schemas: Vec<String>,
    pub cache_enabled: bool,
    pub cache_max_days: u32,
}

impl Default for DatabentoConnectionConfig {
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

/// Rithmic-specific connection configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RithmicConnectionConfig {
    pub environment: RithmicEnvironment,
    #[serde(default)]
    pub server: RithmicServer,
    pub system_name: String,
    pub user_id: String,
    pub auto_reconnect: bool,
    pub subscribed_tickers: Vec<String>,
    #[serde(default = "default_backfill_days")]
    pub backfill_days: i64,
}

fn default_backfill_days() -> i64 {
    1
}

impl Default for RithmicConnectionConfig {
    fn default() -> Self {
        Self {
            environment: RithmicEnvironment::Demo,
            server: RithmicServer::default(),
            system_name: String::new(),
            user_id: String::new(),
            auto_reconnect: true,
            subscribed_tickers: vec![],
            backfill_days: 1,
        }
    }
}
