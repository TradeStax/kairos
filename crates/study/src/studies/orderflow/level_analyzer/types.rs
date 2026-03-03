//! Domain types for the Level Analyzer study.
//!
//! Defines level sources, statuses, monitored levels, touch events,
//! and the top-level data struct exposed to the UI via `interactive_data()`.

use serde::{Deserialize, Serialize};

pub use super::session::{SessionKey, SessionType};

/// Where a level was detected from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LevelSource {
    Hvn,
    Lvn,
    Poc,
    Vah,
    Val,
    SessionHigh,
    SessionLow,
    PriorDayHigh,
    PriorDayLow,
    PriorDayClose,
    HighDeltaZone,
    LowDeltaZone,
    OpeningRangeHigh,
    OpeningRangeLow,
    Manual,
}

impl LevelSource {
    /// Short label for chart display.
    pub fn label(self) -> &'static str {
        match self {
            Self::Hvn => "HVN",
            Self::Lvn => "LVN",
            Self::Poc => "POC",
            Self::Vah => "VAH",
            Self::Val => "VAL",
            Self::SessionHigh => "SH",
            Self::SessionLow => "SL",
            Self::PriorDayHigh => "PDH",
            Self::PriorDayLow => "PDL",
            Self::PriorDayClose => "PDC",
            Self::HighDeltaZone => "HD",
            Self::LowDeltaZone => "LD",
            Self::OpeningRangeHigh => "ORH",
            Self::OpeningRangeLow => "ORL",
            Self::Manual => "MAN",
        }
    }

    /// Priority for deduplication — higher wins.
    pub fn priority(self) -> u8 {
        match self {
            Self::Manual => 10,
            Self::Poc => 9,
            Self::Vah | Self::Val => 8,
            Self::PriorDayHigh | Self::PriorDayLow | Self::PriorDayClose => 7,
            Self::SessionHigh | Self::SessionLow => 6,
            Self::OpeningRangeHigh | Self::OpeningRangeLow => 5,
            Self::Hvn => 4,
            Self::Lvn => 3,
            Self::HighDeltaZone | Self::LowDeltaZone => 2,
        }
    }
}

/// Behavioral status of a monitored level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelStatus {
    /// Not yet visited by price.
    Untested,
    /// Tested and price rejected away.
    Holding,
    /// Price currently within tolerance zone.
    BeingTested,
    /// Multiple tests with diminishing rejection strength.
    Weakening,
    /// Price traded through the level.
    Broken,
}

impl LevelStatus {
    /// Numeric ordering for sort comparisons.
    pub fn order(self) -> u8 {
        match self {
            Self::BeingTested => 0,
            Self::Holding => 1,
            Self::Untested => 2,
            Self::Weakening => 3,
            Self::Broken => 4,
        }
    }

    /// Opacity multiplier for rendering.
    pub fn opacity_multiplier(self) -> f32 {
        match self {
            Self::Untested => 0.8,
            Self::Holding => 1.0,
            Self::BeingTested => 1.0,
            Self::Weakening => 0.7,
            Self::Broken => 0.3,
        }
    }
}

/// Aggregated block fill detected within a level's tolerance zone.
#[derive(Debug, Clone)]
pub struct BlockEvent {
    pub time: u64,
    pub quantity: f64,
    pub is_buy: bool,
    pub fill_count: u32,
}

/// Accumulates same-side fills within aggregation window.
#[derive(Debug, Clone)]
pub struct PendingBlock {
    pub start_time: u64,
    pub last_fill_time: u64,
    pub quantity: f64,
    pub fill_count: u32,
    pub is_buy: bool,
}

/// Cumulative buyer/seller flow metrics for a level.
#[derive(Debug, Clone, Default)]
pub struct FlowMetrics {
    pub buy_volume: f64,
    pub sell_volume: f64,
    pub block_buy_volume: f64,
    pub block_sell_volume: f64,
    pub block_count: u32,
    /// Absorption ratio: passive volume / aggressive volume.
    /// Values above 1.0 mean passive side is absorbing aggression (level defended).
    /// Computed per-touch and exponentially smoothed.
    pub absorption_ratio: f64,
}

/// A single interaction event where price tested a level.
#[derive(Debug, Clone)]
pub struct TouchEvent {
    pub start_time: u64,
    pub end_time: u64,
    pub volume: f64,
    pub delta: f64,
    pub held: bool,
    pub max_excursion_ticks: i32,
    pub rejection_velocity: f64,
    pub buy_volume: f64,
    pub sell_volume: f64,
    pub blocks: Vec<BlockEvent>,
    /// Touch quality score in [0.0, 1.0].
    pub quality_score: f32,
    /// Trades per second leading into touch.
    pub approach_velocity: f64,
}

/// In-progress touch while price is within tolerance.
#[derive(Debug, Clone)]
pub struct ActiveTouch {
    pub start_time: u64,
    pub volume: f64,
    pub delta: f64,
    pub max_excursion_ticks: i32,
    pub last_trade_time: u64,
    pub buy_volume: f64,
    pub sell_volume: f64,
    pub pending_block: Option<PendingBlock>,
    pub blocks: Vec<BlockEvent>,
}

/// A price level being monitored for behavioral analysis.
#[derive(Debug, Clone)]
pub struct MonitoredLevel {
    pub id: u64,
    pub price_units: i64,
    pub price: f64,
    pub source: LevelSource,
    pub status: LevelStatus,
    pub detected_at: u64,
    pub session_key: SessionKey,
    pub touch_count: u32,
    pub hold_count: u32,
    pub break_count: u32,
    pub total_volume_absorbed: f64,
    pub net_delta: f64,
    pub delta_per_touch: Vec<f64>,
    pub time_at_level: u64,
    pub touches: Vec<TouchEvent>,
    pub active_touch: Option<ActiveTouch>,
    pub flow: FlowMetrics,
    /// Composite strength score in [0.0, 1.0].
    pub strength: f32,
}

impl MonitoredLevel {
    /// Create a new untested level.
    pub fn new(
        id: u64,
        price_units: i64,
        price: f64,
        source: LevelSource,
        detected_at: u64,
        session_key: SessionKey,
    ) -> Self {
        Self {
            id,
            price_units,
            price,
            source,
            status: LevelStatus::Untested,
            detected_at,
            session_key,
            touch_count: 0,
            hold_count: 0,
            break_count: 0,
            total_volume_absorbed: 0.0,
            net_delta: 0.0,
            delta_per_touch: Vec::new(),
            time_at_level: 0,
            touches: Vec::new(),
            active_touch: None,
            flow: FlowMetrics::default(),
            strength: 0.0,
        }
    }
}

/// Request to remove a level (passed via `accept_external_data`).
pub struct LevelRemoval {
    pub price_units: i64,
    pub source: LevelSource,
}

/// Top-level data exposed to UI via `interactive_data()` downcast.
pub struct LevelAnalyzerData {
    pub levels: Vec<MonitoredLevel>,
    pub tolerance_ticks: i64,
    pub current_atr: Option<f64>,
    pub block_threshold: f64,
    pub aggregation_window_ms: u64,
    /// Session keys present in the data (for UI filtering).
    pub sessions: Vec<SessionKey>,
}
