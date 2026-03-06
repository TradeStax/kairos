//! VBP parameter definitions and default color constants.
//!
//! Contains the full set of `ParameterDef` arrays for all VBP
//! tabs (Data, Style, POC, Value Area, Peak & Valley, VWAP)
//! and the default color constants used across the study.

mod data;
mod nodes;
mod poc;
mod style;
mod value_area;
mod vwap;

use crate::config::ParameterDef;
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use ::data::SerializableColor;

// ── Default colors ───────────────────────────────────────────

/// Orange, semi-transparent -- total volume bars.
pub(super) const DEFAULT_VOLUME_COLOR: SerializableColor = SerializableColor {
    r: 0.95,
    g: 0.55,
    b: 0.15,
    a: 0.7,
};
/// Bearish color at 70% opacity -- bid (buy) volume.
pub(super) const DEFAULT_BID_COLOR: SerializableColor = BEARISH_COLOR.with_alpha(0.7);
/// Bullish color at 70% opacity -- ask (sell) volume.
pub(super) const DEFAULT_ASK_COLOR: SerializableColor = BULLISH_COLOR.with_alpha(0.7);
/// Gold, fully opaque -- POC line.
pub(super) const DEFAULT_POC_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 0.84,
    b: 0.0,
    a: 1.0,
};
/// Gold at 50% opacity -- developing POC line.
pub(super) const DEFAULT_DEV_POC_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 0.84,
    b: 0.0,
    a: 0.5,
};
/// Cyan at 80% -- Value Area High line.
pub(super) const DEFAULT_VAH_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.7,
    b: 1.0,
    a: 0.8,
};
/// Cyan at 80% -- Value Area Low line.
pub(super) const DEFAULT_VAL_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.7,
    b: 1.0,
    a: 0.8,
};
/// Cyan at 15% -- Value Area fill.
pub(super) const DEFAULT_VA_FILL_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.7,
    b: 1.0,
    a: 0.15,
};
/// Bullish color at 80% -- peak line.
pub(super) const DEFAULT_PEAK_COLOR: SerializableColor = BULLISH_COLOR.with_alpha(0.8);
/// Bullish color at 50% -- developing peak line.
pub(super) const DEFAULT_DEV_PEAK_COLOR: SerializableColor = BULLISH_COLOR.with_alpha(0.5);
/// Bullish color at 50% -- HVN zone fill.
pub(super) const DEFAULT_HVN_ZONE_COLOR: SerializableColor = BULLISH_COLOR.with_alpha(0.5);
/// Bearish color at 80% -- valley line.
pub(super) const DEFAULT_VALLEY_COLOR: SerializableColor = BEARISH_COLOR.with_alpha(0.8);
/// Bearish color at 50% -- developing valley line.
pub(super) const DEFAULT_DEV_VALLEY_COLOR: SerializableColor = BEARISH_COLOR.with_alpha(0.5);
/// Bearish color at 50% -- LVN zone fill.
pub(super) const DEFAULT_LVN_ZONE_COLOR: SerializableColor = BEARISH_COLOR.with_alpha(0.5);
/// Cyan, fully opaque -- anchored VWAP line.
pub(super) const DEFAULT_VWAP_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.9,
    b: 0.9,
    a: 1.0,
};
/// Cyan at 40% -- VWAP standard deviation bands.
pub(super) const DEFAULT_VWAP_BAND_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.9,
    b: 0.9,
    a: 0.4,
};

/// Build the full parameter definition list for VbpStudy.
///
/// Delegates to per-tab helper functions for readability. Each
/// helper appends its parameters to the shared `params` vec.
pub(super) fn build_params() -> Vec<ParameterDef> {
    let mut params = Vec::with_capacity(72);
    data::build_data_tab_params(&mut params);
    style::build_style_tab_params(&mut params);
    poc::build_poc_tab_params(&mut params);
    value_area::build_value_area_tab_params(&mut params);
    nodes::build_nodes_tab_params(&mut params);
    vwap::build_vwap_tab_params(&mut params);
    params
}
