//! Application State
//!
//! Application-level state that is persisted across sessions.
//! Does NOT include chart data (which is derived from cache).

use super::layout::LayoutManager;
use super::persistence::StateVersion;
use crate::config::ScaleFactor;
use crate::config::sidebar::Sidebar;
use crate::config::theme::Theme;
use crate::config::timezone::UserTimezone;
use data::ConnectionManager;
use data::DownloadedTickersRegistry;
use serde::{Deserialize, Serialize};

/// Window specification (position and size)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSpec {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowSpec {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: 1200,
            height: 800,
        }
    }
}

impl WindowSpec {
    pub fn position_coords(self) -> (f32, f32) {
        (self.x.unwrap_or(0) as f32, self.y.unwrap_or(0) as f32)
    }

    pub fn size_coords(self) -> (f32, f32) {
        (self.width as f32, self.height as f32)
    }
}

/// Databento application configuration settings
#[derive(Clone, Serialize, Deserialize)]
pub struct DatabentoAppConfig {
    /// Cache settings
    pub cache_enabled: bool,
    pub cache_max_days: u32,

    /// Live streaming enabled
    pub live_enabled: bool,
}

impl Default for DatabentoAppConfig {
    fn default() -> Self {
        Self {
            cache_enabled: true,
            cache_max_days: 90,
            live_enabled: false,
        }
    }
}

/// Auto-update preferences (persisted)
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoUpdatePreferences {
    pub auto_check_enabled: bool,
    pub check_interval_hours: u32,
    pub last_check_epoch: Option<i64>,
    pub skipped_versions: Vec<String>,
}

impl Default for AutoUpdatePreferences {
    fn default() -> Self {
        Self {
            auto_check_enabled: true,
            check_interval_hours: 24,
            last_check_epoch: None,
            skipped_versions: vec![],
        }
    }
}

/// AI assistant preferences (persisted)
#[derive(Clone, Serialize, Deserialize)]
pub struct AiPreferences {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl Default for AiPreferences {
    fn default() -> Self {
        Self {
            model: "google/gemini-3-flash-preview".to_string(),
            temperature: 0.3,
            max_tokens: 4096,
        }
    }
}

/// Application state (persisted to app-state.json)
///
/// This contains ONLY application-level preferences and configuration.
/// Chart data is NOT persisted here - it is derived from cache.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    /// Version of the state schema (for migrations)
    pub version: StateVersion,

    /// Layout management (panes, splits, active layout)
    pub layout_manager: LayoutManager,

    /// UI theme
    pub selected_theme: Theme,

    /// Custom theme (if using custom)
    pub custom_theme: Option<Theme>,

    /// Main window specification (position, size)
    pub main_window: Option<WindowSpec>,

    /// Timezone configuration
    pub timezone: UserTimezone,

    /// Sidebar state
    pub sidebar: Sidebar,

    /// UI scale factor
    pub scale_factor: ScaleFactor,

    /// Whether trade fetching is enabled (global toggle)
    pub trade_fetch_enabled: bool,

    /// Databento configuration
    /// NOTE: API key is read from environment variables only (DATABENTO_API_KEY)
    pub databento_config: DatabentoAppConfig,

    /// Registry of downloaded tickers with their date ranges
    pub downloaded_tickers: DownloadedTickersRegistry,

    /// Data connections
    #[serde(default)]
    pub data_feeds: ConnectionManager,

    /// AI assistant preferences
    #[serde(default)]
    pub ai_preferences: AiPreferences,

    /// Auto-update preferences
    #[serde(default)]
    pub auto_update: AutoUpdatePreferences,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            version: StateVersion::CURRENT,
            layout_manager: LayoutManager::default(),
            selected_theme: Theme::default(),
            custom_theme: None,
            main_window: None,
            timezone: UserTimezone::default(),
            sidebar: Sidebar::default(),
            scale_factor: ScaleFactor::default(),
            trade_fetch_enabled: false,
            databento_config: DatabentoAppConfig::default(),
            downloaded_tickers: DownloadedTickersRegistry::default(),
            data_feeds: ConnectionManager::default(),
            ai_preferences: AiPreferences::default(),
            auto_update: AutoUpdatePreferences::default(),
        }
    }
}

impl AppState {
    /// Create from parts (for backward compatibility)
    pub fn from_parts(
        layout_manager: LayoutManager,
        selected_theme: Theme,
        custom_theme: Option<Theme>,
        main_window: Option<WindowSpec>,
        timezone: UserTimezone,
        sidebar: Sidebar,
        scale_factor: ScaleFactor,
        downloaded_tickers: DownloadedTickersRegistry,
        data_feeds: ConnectionManager,
    ) -> Self {
        Self {
            version: StateVersion::CURRENT,
            layout_manager,
            selected_theme,
            custom_theme,
            main_window,
            timezone,
            sidebar,
            scale_factor,
            trade_fetch_enabled: false,
            databento_config: DatabentoAppConfig::default(),
            downloaded_tickers,
            data_feeds,
            ai_preferences: AiPreferences::default(),
            auto_update: AutoUpdatePreferences::default(),
        }
    }

    /// Get current state version
    pub fn schema_version(&self) -> u32 {
        self.version.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        assert_eq!(state.version, StateVersion::CURRENT);
        assert!(state.databento_config.cache_enabled);
        assert_eq!(state.databento_config.cache_max_days, 90);
        assert!(!state.databento_config.live_enabled);
    }

    #[test]
    fn test_layout_manager() {
        let manager = LayoutManager::default();
        assert!(manager.layouts.is_empty());
        assert!(manager.active_layout.is_none());
    }

    // ── WindowSpec ──────────────────────────────────────────

    #[test]
    fn test_window_spec_default() {
        let spec = WindowSpec::default();
        assert_eq!(spec.width, 1200);
        assert_eq!(spec.height, 800);
        assert!(spec.x.is_none());
        assert!(spec.y.is_none());
    }

    #[test]
    fn test_window_spec_position_coords_with_position() {
        let spec = WindowSpec {
            x: Some(100),
            y: Some(200),
            width: 800,
            height: 600,
        };
        let (x, y) = spec.position_coords();
        assert!((x - 100.0).abs() < 0.01);
        assert!((y - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_window_spec_position_coords_without_position() {
        let spec = WindowSpec::default();
        let (x, y) = spec.position_coords();
        assert!((x - 0.0).abs() < 0.01);
        assert!((y - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_window_spec_size_coords() {
        let spec = WindowSpec {
            x: None,
            y: None,
            width: 1920,
            height: 1080,
        };
        let (w, h) = spec.size_coords();
        assert!((w - 1920.0).abs() < 0.01);
        assert!((h - 1080.0).abs() < 0.01);
    }

    #[test]
    fn test_window_spec_serialization_roundtrip() {
        let spec = WindowSpec {
            x: Some(50),
            y: Some(75),
            width: 1600,
            height: 900,
        };
        let json = serde_json::to_string(&spec).unwrap();
        let loaded: WindowSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.x, Some(50));
        assert_eq!(loaded.y, Some(75));
        assert_eq!(loaded.width, 1600);
        assert_eq!(loaded.height, 900);
    }

    // ── DatabentoAppConfig ──────────────────────────────────

    #[test]
    fn test_databento_config_default() {
        let config = DatabentoAppConfig::default();
        assert!(config.cache_enabled);
        assert_eq!(config.cache_max_days, 90);
        assert!(!config.live_enabled);
    }

    #[test]
    fn test_databento_config_serialization_roundtrip() {
        let config = DatabentoAppConfig {
            cache_enabled: false,
            cache_max_days: 365,
            live_enabled: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: DatabentoAppConfig = serde_json::from_str(&json).unwrap();
        assert!(!loaded.cache_enabled);
        assert_eq!(loaded.cache_max_days, 365);
        assert!(loaded.live_enabled);
    }

    // ── AiPreferences ───────────────────────────────────────

    #[test]
    fn test_ai_preferences_default() {
        let prefs = AiPreferences::default();
        assert!(!prefs.model.is_empty());
        assert!(prefs.temperature > 0.0 && prefs.temperature < 2.0);
        assert!(prefs.max_tokens > 0);
    }

    #[test]
    fn test_ai_preferences_serialization_roundtrip() {
        let prefs = AiPreferences {
            model: "test-model".to_string(),
            temperature: 0.9,
            max_tokens: 8192,
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: AiPreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.model, "test-model");
        assert!((loaded.temperature - 0.9).abs() < 0.01);
        assert_eq!(loaded.max_tokens, 8192);
    }

    // ── AppState ────────────────────────────────────────────

    #[test]
    fn test_app_state_schema_version() {
        let state = AppState::default();
        assert_eq!(state.schema_version(), StateVersion::CURRENT.0);
    }

    #[test]
    fn test_app_state_serialization_roundtrip() {
        let state = AppState::default();
        let json = serde_json::to_string(&state).unwrap();
        let loaded: AppState = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.version, state.version);
        assert_eq!(loaded.trade_fetch_enabled, state.trade_fetch_enabled);
    }

    #[test]
    fn test_app_state_serde_default_fills_missing_fields() {
        // Minimal JSON with only version — serde(default) should fill rest
        let json = r#"{"version":1}"#;
        let loaded: AppState = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.version, StateVersion(1));
        assert!(loaded.databento_config.cache_enabled);
    }
}
