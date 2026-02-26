//! Volume profile output and VBP configuration types.
//!
//! - [`types`] — Core profile primitives: levels, sides, nodes, directions.
//! - [`vbp`] — VBP-specific configuration: POC, Value Area, HVN/LVN, VWAP.

pub mod types;
pub mod vbp;

pub use types::{ExtendDirection, NodeDetectionMethod, ProfileLevel, ProfileSide, VolumeNode};
pub use vbp::{
    VbpGroupingMode, VbpNodeConfig, VbpPeriod, VbpPocConfig, VbpResolvedCache, VbpSplitPeriod,
    VbpType, VbpValueAreaConfig, VbpVwapConfig,
};

use data::SerializableColor;
use serde::{Deserialize, Serialize};

/// Computed volume profile data -- pure data, no rendering config.
#[derive(Debug)]
pub struct ProfileOutput {
    pub levels: Vec<ProfileLevel>,
    pub quantum: i64,
    pub poc: Option<usize>,
    pub value_area: Option<(usize, usize)>,
    pub time_range: Option<(u64, u64)>,
    // Node detection results (empty for simple profiles)
    pub hvn_zones: Vec<(i64, i64)>,
    pub lvn_zones: Vec<(i64, i64)>,
    pub peak_node: Option<VolumeNode>,
    pub valley_node: Option<VolumeNode>,
    // Developing series (empty for simple profiles)
    pub developing_poc_points: Vec<(u64, i64)>,
    pub developing_peak_points: Vec<(u64, i64)>,
    pub developing_valley_points: Vec<(u64, i64)>,
    // VWAP (empty for simple profiles)
    pub vwap_points: Vec<(u64, f32)>,
    pub vwap_upper_points: Vec<(u64, f32)>,
    pub vwap_lower_points: Vec<(u64, f32)>,
    pub grouping_mode: VbpGroupingMode,
    /// Renderer-side cache for resolved (merged) levels.
    /// Populated lazily by the renderer; avoids per-frame merging.
    pub resolved_cache: std::sync::Arc<std::sync::Mutex<Option<VbpResolvedCache>>>,
}

impl Clone for ProfileOutput {
    fn clone(&self) -> Self {
        Self {
            levels: self.levels.clone(),
            quantum: self.quantum,
            poc: self.poc,
            value_area: self.value_area,
            time_range: self.time_range,
            hvn_zones: self.hvn_zones.clone(),
            lvn_zones: self.lvn_zones.clone(),
            peak_node: self.peak_node.clone(),
            valley_node: self.valley_node.clone(),
            developing_poc_points: self.developing_poc_points.clone(),
            developing_peak_points: self.developing_peak_points.clone(),
            developing_valley_points: self.developing_valley_points.clone(),
            vwap_points: self.vwap_points.clone(),
            vwap_upper_points: self.vwap_upper_points.clone(),
            vwap_lower_points: self.vwap_lower_points.clone(),
            grouping_mode: self.grouping_mode,
            resolved_cache: std::sync::Arc::clone(&self.resolved_cache),
        }
    }
}

impl ProfileOutput {
    pub fn empty() -> Self {
        Self {
            levels: Vec::new(),
            quantum: 1,
            poc: None,
            value_area: None,
            time_range: None,
            hvn_zones: Vec::new(),
            lvn_zones: Vec::new(),
            peak_node: None,
            valley_node: None,
            developing_poc_points: Vec::new(),
            developing_peak_points: Vec::new(),
            developing_valley_points: Vec::new(),
            vwap_points: Vec::new(),
            vwap_upper_points: Vec::new(),
            vwap_lower_points: Vec::new(),
            grouping_mode: VbpGroupingMode::Manual,
            resolved_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }
}

/// Rendering configuration for volume profiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRenderConfig {
    pub vbp_type: VbpType,
    pub side: ProfileSide,
    pub width_pct: f32,
    pub opacity: f32,
    pub volume_color: SerializableColor,
    pub bid_color: SerializableColor,
    pub ask_color: SerializableColor,
    pub poc_config: VbpPocConfig,
    pub va_config: VbpValueAreaConfig,
    pub node_config: VbpNodeConfig,
    pub vwap_config: VbpVwapConfig,
}

impl ProfileRenderConfig {
    /// Simple profile rendering config (for VolumeProfileStudy).
    pub fn simple(
        side: ProfileSide,
        width_pct: f32,
        buy_color: SerializableColor,
        sell_color: SerializableColor,
        poc_color: SerializableColor,
        value_area_color: SerializableColor,
    ) -> Self {
        Self {
            vbp_type: VbpType::BidAskVolume,
            side,
            width_pct,
            opacity: 1.0,
            volume_color: buy_color,
            bid_color: buy_color,
            ask_color: sell_color,
            poc_config: VbpPocConfig {
                show_poc: true,
                poc_color,
                ..Default::default()
            },
            va_config: VbpValueAreaConfig {
                show_value_area: true,
                va_fill_color: value_area_color,
                ..Default::default()
            },
            node_config: VbpNodeConfig::default(),
            vwap_config: VbpVwapConfig::default(),
        }
    }
}
