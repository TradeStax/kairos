//! State Management - Clean Separation of Concerns
//!
//! This module provides proper state management with clear separation between:
//! - Application state (theme, UI preferences, window specs)
//! - Chart state (chart configuration, not data)
//! - Persistence layer (load/save with versioning)
//!
//! ## Architecture
//!
//! ```text
//! AppState (persisted to saved-state.json)
//!   ├─ UI preferences (theme, timezone, scale)
//!   ├─ Window configuration
//!   ├─ Layout management
//!   └─ Audio settings
//!
//! ChartState (in-memory only, NOT persisted)
//!   ├─ Chart configuration
//!   ├─ Loading status
//!   └─ Current data (trades, candles)
//!
//! Persistence Layer
//!   ├─ Versioned serialization
//!   ├─ Migration support
//!   └─ Backup on failure
//! ```
//!
//! ## Key Principle
//! - App state: Small, persisted (theme, layout, preferences)
//! - Chart data: Large, NOT persisted (derives from cache)

pub mod app_state;
pub mod chart_state;
pub mod downloaded_tickers;
pub mod layout_manager;
pub mod layout_types;
pub mod migrations;
pub mod pane;
pub mod pane_config;
pub mod persistence;
pub mod replay_state;

pub use app_state::{AppState, WindowSpec};
pub use chart_state::ChartState;
pub use downloaded_tickers::DownloadedTickersRegistry;
pub use layout_manager::{Layout, LayoutManager, Layouts};
pub use layout_types::{Axis, Dashboard, Pane};
pub use pane_config::{
    ComparisonConfig, ContentKind, HeatmapConfig, KlineConfig, LadderConfig, LinkGroup, Settings,
    TimeAndSalesConfig, VisualConfig,
};
pub use persistence::{StateVersion, load_state, save_state};
pub use replay_state::{PlaybackStatus, ReplayData, ReplayState, SpeedPreset};
