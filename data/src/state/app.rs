//! Application State
//!
//! Application-level state that is persisted across sessions.
//! Does NOT include chart data (which is derived from cache).

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

/// Layout definition
#[derive(Clone, Serialize, Deserialize)]
pub struct Layout {
    pub name: Option<String>,
    pub panes: Vec<String>, // Simplified - actual pane data would be more complex
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

/// Application state (persisted to saved-state.json)
///
/// This contains ONLY application-level preferences and configuration.
/// Chart data is NOT persisted here - it is derived from cache.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    /// Version of the state schema (for migrations)
    pub version: u32,

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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            version: 2,
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
            version: 2,
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
        }
    }

    /// Get current state version
    pub fn schema_version(&self) -> u32 {
        self.version
    }
}

/// Layout manager state
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct LayoutManager {
    pub layouts: Vec<Layout>,
    pub active_layout: Option<String>,
}

impl LayoutManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_layout(&mut self, layout: Layout) {
        self.layouts.push(layout);
    }

    pub fn set_active(&mut self, name: String) {
        self.active_layout = Some(name);
    }

    pub fn get_active(&self) -> Option<&Layout> {
        self.active_layout.as_ref().and_then(|name| {
            self.layouts.iter().find(|l| {
                if let Some(layout_name) = &l.name {
                    layout_name == name
                } else {
                    false
                }
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        assert_eq!(state.version, 2);
        assert!(state.databento_config.cache_enabled);
        assert_eq!(state.databento_config.cache_max_days, 90);
        assert!(!state.databento_config.live_enabled);
    }

    #[test]
    fn test_layout_manager() {
        let manager = LayoutManager::new();
        assert!(manager.layouts.is_empty());
        assert!(manager.active_layout.is_none());
    }
}
