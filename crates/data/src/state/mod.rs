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
//!   └─ Layout management
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

pub mod app;
pub mod chart;
pub mod layout;
pub mod migrations;
pub mod pane;
pub mod persistence;
pub mod registry;
pub mod replay;

pub use app::{AppState, WindowSpec};
pub use chart::ChartState;
pub use layout::{Axis, Dashboard, Layout, LayoutManager, Layouts, Pane};
pub use pane::{
    ComparisonConfig, ContentKind, HeatmapConfig, KlineConfig, LadderConfig, LinkGroup,
    ScriptEditorConfig, Settings, StudyInstanceConfig, TimeAndSalesConfig, VisualConfig,
};
pub use persistence::{StateVersion, load_state, save_state};
pub use registry::DownloadedTickersRegistry;
pub use replay::{PlaybackStatus, ReplayData, ReplayState, SpeedPreset};
