//! VBP config import/export and config builder helpers.
//!
//! Contains methods for building nested render configs from
//! parameter values, string-to-enum parsing helpers, and
//! JSON import/export for drawing serialization.

use crate::config::{LineStyleValue, StudyConfig};
use crate::output::{
    ExtendDirection, NodeDetectionMethod, ProfileSide, VbpNodeConfig, VbpPeriod, VbpPocConfig,
    VbpSplitPeriod, VbpType, VbpValueAreaConfig, VbpVwapConfig,
};

use super::VbpStudy;
use super::params::*;

impl VbpStudy {
    /// Export config values as JSON for drawing serialization.
    pub fn export_config(&self) -> serde_json::Value {
        serde_json::to_value(&self.config).unwrap_or_default()
    }

    /// Import config values from JSON (drawing deserialization).
    pub fn import_config(&mut self, value: &serde_json::Value) {
        if let Ok(config) = serde_json::from_value::<StudyConfig>(value.clone()) {
            self.config = config;
            self.last_input_fingerprint = (0, 0, 0, 0, 0);
        }
    }

    // ── String-to-enum parsing helpers ────────────────────────

    pub(super) fn parse_vbp_type(s: &str) -> VbpType {
        match s {
            "Bid/Ask Volume" => VbpType::BidAskVolume,
            "Delta" => VbpType::Delta,
            "Delta & Total Volume" => VbpType::DeltaAndTotalVolume,
            "Delta Percentage" => VbpType::DeltaPercentage,
            _ => VbpType::Volume,
        }
    }

    pub(super) fn parse_period(s: &str) -> VbpPeriod {
        match s {
            "Custom" => VbpPeriod::Custom,
            _ => VbpPeriod::Split,
        }
    }

    /// Parse the split_interval + split_unit/split_value
    /// params into a [`VbpSplitPeriod`].
    pub(super) fn parse_split_period(&self) -> VbpSplitPeriod {
        let interval = self.config.get_choice("split_interval", "1 Day");
        match interval {
            "1 Day" => VbpSplitPeriod::Day,
            "4 Hours" => VbpSplitPeriod::Hours(4),
            "2 Hours" => VbpSplitPeriod::Hours(2),
            "1 Hour" => VbpSplitPeriod::Hours(1),
            "30 Minutes" => VbpSplitPeriod::Minutes(30),
            "15 Minutes" => VbpSplitPeriod::Minutes(15),
            "Custom" => {
                let unit = self.config.get_choice("split_unit", "Hours");
                let value = self.config.get_int("split_value", 1).max(1) as u32;
                match unit {
                    "Days" => VbpSplitPeriod::Hours(value * 24),
                    "Minutes" => VbpSplitPeriod::Minutes(value),
                    "Contracts" => VbpSplitPeriod::Contracts(value),
                    _ => VbpSplitPeriod::Hours(value),
                }
            }
            _ => VbpSplitPeriod::Day,
        }
    }

    pub(super) fn parse_side(s: &str) -> ProfileSide {
        match s {
            "Right" => ProfileSide::Right,
            _ => ProfileSide::Left,
        }
    }

    pub(super) fn parse_extend(s: &str) -> ExtendDirection {
        match s {
            "Left" => ExtendDirection::Left,
            "Right" => ExtendDirection::Right,
            "Both" => ExtendDirection::Both,
            _ => ExtendDirection::None,
        }
    }

    pub(super) fn parse_node_method(s: &str) -> NodeDetectionMethod {
        match s {
            "Relative" => NodeDetectionMethod::Relative,
            "Std Dev" => NodeDetectionMethod::StdDev,
            _ => NodeDetectionMethod::Percentile,
        }
    }

    // ── Config builders ───────────────────────────────────────

    /// Build POC config from current parameter values.
    pub(super) fn build_poc_config(&self) -> VbpPocConfig {
        VbpPocConfig {
            show_poc: self.config.get_bool("poc_show", true),
            poc_color: self.config.get_color("poc_color", DEFAULT_POC_COLOR),
            poc_line_width: self.config.get_float("poc_line_width", 1.5) as f32,
            poc_line_style: self
                .config
                .get_line_style("poc_line_style", LineStyleValue::Solid),
            poc_extend: Self::parse_extend(self.config.get_choice("poc_extend", "None")),
            show_poc_label: self.config.get_bool("poc_show_label", false),
            show_developing_poc: self.config.get_bool("poc_show_developing", false),
            developing_poc_color: self
                .config
                .get_color("poc_dev_color", DEFAULT_DEV_POC_COLOR),
            developing_poc_line_width: self.config.get_float("poc_dev_line_width", 1.0) as f32,
            developing_poc_line_style: self
                .config
                .get_line_style("poc_dev_line_style", LineStyleValue::Dashed),
        }
    }

    /// Build Value Area config from current parameter values.
    pub(super) fn build_va_config(&self) -> VbpValueAreaConfig {
        VbpValueAreaConfig {
            show_value_area: self.config.get_bool("va_show", true),
            value_area_pct: self.config.get_float("value_area_pct", 0.7) as f32,
            show_va_highlight: self.config.get_bool("va_show_highlight", true),
            vah_color: self.config.get_color("va_vah_color", DEFAULT_VAH_COLOR),
            vah_line_width: self.config.get_float("va_vah_line_width", 1.0) as f32,
            vah_line_style: self
                .config
                .get_line_style("va_vah_line_style", LineStyleValue::Dashed),
            val_color: self.config.get_color("va_val_color", DEFAULT_VAL_COLOR),
            val_line_width: self.config.get_float("va_val_line_width", 1.0) as f32,
            val_line_style: self
                .config
                .get_line_style("va_val_line_style", LineStyleValue::Dashed),
            show_va_fill: self.config.get_bool("va_show_fill", false),
            va_fill_color: self
                .config
                .get_color("va_fill_color", DEFAULT_VA_FILL_COLOR),
            va_fill_opacity: self.config.get_float("va_fill_opacity", 0.15) as f32,
            va_extend: Self::parse_extend(self.config.get_choice("va_extend", "None")),
            show_va_labels: self.config.get_bool("va_show_labels", false),
        }
    }

    /// Build Node config from current parameter values.
    pub(super) fn build_node_config(&self) -> VbpNodeConfig {
        VbpNodeConfig {
            hvn_method: Self::parse_node_method(
                self.config.get_choice("node_hvn_method", "Percentile"),
            ),
            hvn_threshold: self.config.get_float("node_hvn_threshold", 0.85) as f32,
            lvn_method: Self::parse_node_method(
                self.config.get_choice("node_lvn_method", "Percentile"),
            ),
            lvn_threshold: self.config.get_float("node_lvn_threshold", 0.15) as f32,
            min_prominence: self.config.get_float("node_min_prominence", 0.15) as f32,

            show_hvn_zones: self.config.get_bool("hvn_zone_show", false),
            hvn_zone_color: self
                .config
                .get_color("hvn_zone_color", DEFAULT_HVN_ZONE_COLOR),
            hvn_zone_opacity: self.config.get_float("hvn_zone_opacity", 0.08) as f32,

            show_peak_line: self.config.get_bool("peak_show", false),
            peak_color: self.config.get_color("peak_color", DEFAULT_PEAK_COLOR),
            peak_line_style: self
                .config
                .get_line_style("peak_line_style", LineStyleValue::Solid),
            peak_line_width: self.config.get_float("peak_line_width", 1.5) as f32,
            peak_extend: Self::parse_extend(self.config.get_choice("peak_extend", "None")),
            show_peak_label: self.config.get_bool("peak_show_label", false),

            show_developing_peak: self.config.get_bool("dev_peak_show", false),
            developing_peak_color: self
                .config
                .get_color("dev_peak_color", DEFAULT_DEV_PEAK_COLOR),
            developing_peak_line_width: self.config.get_float("dev_peak_line_width", 1.0) as f32,
            developing_peak_line_style: self
                .config
                .get_line_style("dev_peak_line_style", LineStyleValue::Dashed),

            show_lvn_zones: self.config.get_bool("lvn_zone_show", false),
            lvn_zone_color: self
                .config
                .get_color("lvn_zone_color", DEFAULT_LVN_ZONE_COLOR),
            lvn_zone_opacity: self.config.get_float("lvn_zone_opacity", 0.08) as f32,

            show_valley_line: self.config.get_bool("valley_show", false),
            valley_color: self.config.get_color("valley_color", DEFAULT_VALLEY_COLOR),
            valley_line_style: self
                .config
                .get_line_style("valley_line_style", LineStyleValue::Solid),
            valley_line_width: self.config.get_float("valley_line_width", 1.5) as f32,
            valley_extend: Self::parse_extend(self.config.get_choice("valley_extend", "None")),
            show_valley_label: self.config.get_bool("valley_show_label", false),

            show_developing_valley: self.config.get_bool("dev_valley_show", false),
            developing_valley_color: self
                .config
                .get_color("dev_valley_color", DEFAULT_DEV_VALLEY_COLOR),
            developing_valley_line_width: self.config.get_float("dev_valley_line_width", 1.0)
                as f32,
            developing_valley_line_style: self
                .config
                .get_line_style("dev_valley_line_style", LineStyleValue::Dashed),
        }
    }

    /// Build VWAP config from current parameter values.
    pub(super) fn build_vwap_config(&self) -> VbpVwapConfig {
        VbpVwapConfig {
            show_vwap: self.config.get_bool("vwap_show", false),
            vwap_color: self.config.get_color("vwap_color", DEFAULT_VWAP_COLOR),
            vwap_line_width: self.config.get_float("vwap_line_width", 1.5) as f32,
            vwap_line_style: self
                .config
                .get_line_style("vwap_line_style", LineStyleValue::Solid),
            show_vwap_label: self.config.get_bool("vwap_show_label", false),
            show_bands: self.config.get_bool("vwap_show_bands", false),
            band_multiplier: self.config.get_float("vwap_band_multiplier", 1.0) as f32,
            band_color: self
                .config
                .get_color("vwap_band_color", DEFAULT_VWAP_BAND_COLOR),
            band_line_style: self
                .config
                .get_line_style("vwap_band_line_style", LineStyleValue::Dashed),
            band_line_width: self.config.get_float("vwap_band_line_width", 1.0) as f32,
        }
    }
}
