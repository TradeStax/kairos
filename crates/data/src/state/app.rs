//! Application State
//!
//! Application-level state that is persisted across sessions.
//! Does NOT include chart data (which is derived from cache).

use super::persistence::StateVersion;
use super::registry::DownloadedTickersRegistry;
use crate::config::ScaleFactor;
use crate::config::sidebar::Sidebar;
use crate::config::theme::Theme;
use crate::config::timezone::UserTimezone;
use crate::feed::DataFeedManager;
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

/// Databento configuration settings
#[derive(Clone, Serialize, Deserialize)]
pub struct DatabentoConfig {
    /// Cache settings
    pub cache_enabled: bool,
    pub cache_max_days: u32,

    /// Live streaming enabled
    pub live_enabled: bool,
}

impl Default for DatabentoConfig {
    fn default() -> Self {
        Self {
            cache_enabled: true,
            cache_max_days: 90,
            live_enabled: false,
        }
    }
}

/// Massive (Polygon) options configuration settings
#[derive(Clone, Serialize, Deserialize)]
pub struct MassiveConfigSettings {
    /// Cache settings
    pub cache_enabled: bool,
    pub cache_max_days: u32,

    /// Rate limit (requests per minute)
    pub rate_limit_per_minute: u32,

    /// Request timeout in seconds
    pub timeout_secs: u64,

    /// Options data fetch enabled
    pub options_fetch_enabled: bool,
}

impl Default for MassiveConfigSettings {
    fn default() -> Self {
        Self {
            cache_enabled: true,
            cache_max_days: 90,
            rate_limit_per_minute: 5,
            timeout_secs: 30,
            options_fetch_enabled: false,
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

/// Application state (persisted to saved-state.json)
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
    pub databento_config: DatabentoConfig,

    /// Massive (Polygon) configuration
    /// NOTE: API key is read from environment variables only (MASSIVE_API_KEY)
    pub massive_config: MassiveConfigSettings,

    /// Registry of downloaded tickers with their date ranges
    pub downloaded_tickers: DownloadedTickersRegistry,

    /// Data feed connections
    #[serde(default)]
    pub data_feeds: DataFeedManager,

    /// AI assistant preferences
    #[serde(default)]
    pub ai_preferences: AiPreferences,
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
            databento_config: DatabentoConfig::default(),
            massive_config: MassiveConfigSettings::default(),
            downloaded_tickers: DownloadedTickersRegistry::default(),
            data_feeds: DataFeedManager::default(),
            ai_preferences: AiPreferences::default(),
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
        data_feeds: DataFeedManager,
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
            databento_config: DatabentoConfig::default(),
            massive_config: MassiveConfigSettings::default(),
            downloaded_tickers,
            data_feeds,
            ai_preferences: AiPreferences::default(),
        }
    }

    /// Get current state version
    pub fn schema_version(&self) -> u32 {
        self.version.0
    }
}

/// Re-export the canonical LayoutManager from state::layout.
/// The thin wrapper has been removed; all callers should use this type directly.
pub use super::layout::LayoutManager;

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
}
