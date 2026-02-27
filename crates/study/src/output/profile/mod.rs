//! Volume profile output and VBP configuration types.
//!
//! - [`types`] — Core profile primitives: levels, sides, nodes, directions.
//! - [`vbp`] — VBP-specific configuration: POC, Value Area, HVN/LVN, VWAP.

pub mod types;
pub mod vbp;

pub use types::{
    ExtendDirection, NodeDetectionMethod, ProfileLevel, ProfileSide,
    VolumeNode,
};
pub use vbp::{
    VbpGroupingMode, VbpNodeConfig, VbpPeriod, VbpPocConfig,
    VbpResolvedCache, VbpSplitPeriod, VbpType, VbpValueAreaConfig,
    VbpVwapConfig,
};

use data::SerializableColor;
use serde::{Deserialize, Serialize};

/// Computed volume profile data — pure data, no rendering config.
///
/// Contains the aggregated volume levels, detected nodes, developing
/// series, and VWAP points for a single profile segment.
#[derive(Debug)]
pub struct ProfileOutput {
    /// Volume at each price level.
    pub levels: Vec<ProfileLevel>,
    /// Price quantum (tick size in fixed-point units) used for level
    /// spacing.
    pub quantum: i64,
    /// Index of the Point of Control (highest volume level) in
    /// `levels`.
    pub poc: Option<usize>,
    /// Value area bounds as `(VAH index, VAL index)` in `levels`.
    pub value_area: Option<(usize, usize)>,
    /// Time range covered by this profile as `(start_ms, end_ms)`.
    pub time_range: Option<(u64, u64)>,
    /// Detected HVN zones as `(low_price_units, high_price_units)`
    /// pairs.
    pub hvn_zones: Vec<(i64, i64)>,
    /// Detected LVN zones as `(low_price_units, high_price_units)`
    /// pairs.
    pub lvn_zones: Vec<(i64, i64)>,
    /// Single dominant peak node (highest-volume HVN).
    pub peak_node: Option<VolumeNode>,
    /// Single deepest valley node (lowest-volume LVN).
    pub valley_node: Option<VolumeNode>,
    /// Developing POC series as `(timestamp_ms, price_units)` points.
    pub developing_poc_points: Vec<(u64, i64)>,
    /// Developing peak series as `(timestamp_ms, price_units)` points.
    pub developing_peak_points: Vec<(u64, i64)>,
    /// Developing valley series as `(timestamp_ms, price_units)`
    /// points.
    pub developing_valley_points: Vec<(u64, i64)>,
    /// Anchored VWAP series as `(timestamp_ms, value)` points.
    pub vwap_points: Vec<(u64, f32)>,
    /// Upper VWAP band series as `(timestamp_ms, value)` points.
    pub vwap_upper_points: Vec<(u64, f32)>,
    /// Lower VWAP band series as `(timestamp_ms, value)` points.
    pub vwap_lower_points: Vec<(u64, f32)>,
    /// How levels are grouped (manual or automatic).
    pub grouping_mode: VbpGroupingMode,
    /// Renderer-side cache for dynamically merged levels. Populated
    /// lazily by the renderer to avoid per-frame merging.
    pub resolved_cache:
        std::sync::Arc<std::sync::Mutex<Option<VbpResolvedCache>>>,
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
            peak_node: self.peak_node,
            valley_node: self.valley_node,
            developing_poc_points: self.developing_poc_points.clone(),
            developing_peak_points: self.developing_peak_points.clone(),
            developing_valley_points: self
                .developing_valley_points
                .clone(),
            vwap_points: self.vwap_points.clone(),
            vwap_upper_points: self.vwap_upper_points.clone(),
            vwap_lower_points: self.vwap_lower_points.clone(),
            grouping_mode: self.grouping_mode,
            resolved_cache: std::sync::Arc::clone(&self.resolved_cache),
        }
    }
}

impl ProfileOutput {
    /// Create an empty profile with no levels or nodes.
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
            resolved_cache: std::sync::Arc::new(
                std::sync::Mutex::new(None),
            ),
        }
    }
}

/// Rendering configuration for volume profiles.
///
/// Controls the visual appearance of profile bars, POC line, value area,
/// HVN/LVN nodes, and anchored VWAP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRenderConfig {
    /// Visualization type (volume, bid/ask, delta, etc.).
    pub vbp_type: VbpType,
    /// Which side of the chart to render profile bars.
    pub side: ProfileSide,
    /// Width of the profile region as a percentage of available space.
    pub width_pct: f32,
    /// Overall profile opacity.
    pub opacity: f32,
    /// Color for total volume bars.
    pub volume_color: SerializableColor,
    /// Color for bid-side volume bars.
    pub bid_color: SerializableColor,
    /// Color for ask-side volume bars.
    pub ask_color: SerializableColor,
    /// POC sub-feature configuration.
    pub poc_config: VbpPocConfig,
    /// Value Area sub-feature configuration.
    pub va_config: VbpValueAreaConfig,
    /// HVN/LVN node sub-feature configuration.
    pub node_config: VbpNodeConfig,
    /// Anchored VWAP sub-feature configuration.
    pub vwap_config: VbpVwapConfig,
}

impl ProfileRenderConfig {
    /// Create a simple profile rendering config (for VolumeProfileStudy).
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
