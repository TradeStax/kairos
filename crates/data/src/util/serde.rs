//! Serde utility helpers and default value functions.
//!
//! [`ok_or_default`] provides graceful deserialization fallback. The
//! `default_*` functions are used with `#[serde(default = "...")]` attributes
//! throughout the crate for stable default values during schema evolution.

use serde::{Deserialize, Deserializer};

/// Deserializes a value, falling back to `Default` if parsing fails.
///
/// Useful for forward-compatible deserialization where new enum variants
/// or changed field types should not break loading of saved state.
pub fn ok_or_default<'a, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'a> + Default,
    D: Deserializer<'a>,
{
    let v: serde_json::Value = Deserialize::deserialize(deserializer)?;
    match T::deserialize(v) {
        Ok(val) => Ok(val),
        Err(e) => {
            log::debug!("Deserialization failed, using default: {}", e);
            Ok(T::default())
        }
    }
}

// ── Default value functions for serde ────────────────────────────────────

/// Returns `true`
pub fn default_true() -> bool {
    true
}

/// Returns `1i64`
pub fn default_one() -> i64 {
    1
}

/// Returns `1i64` (default split multiplier)
pub fn default_split_value() -> i64 {
    1
}

/// Returns `20i64` (maximum number of volume profiles)
pub fn default_max_profiles() -> i64 {
    20
}

/// Returns `0.7` (70% value area percentage)
pub fn default_va_pct() -> f32 {
    0.7
}

/// Returns `1.5` (point-of-control line width)
pub fn default_poc_width() -> f32 {
    1.5
}

/// Returns `0.85` (high volume node threshold)
pub fn default_hvn_threshold() -> f32 {
    0.85
}

/// Returns `0.15` (low volume node threshold)
pub fn default_lvn_threshold() -> f32 {
    0.15
}

/// Returns `0.7` (default opacity)
pub fn default_opacity() -> f32 {
    0.7
}

/// Returns `0.08` (value area fill opacity)
pub fn default_va_fill_opacity() -> f32 {
    0.08
}

/// Returns `1.0` (default line width)
pub fn default_line_width() -> f32 {
    1.0
}

/// Returns `0.15` (zone overlay opacity)
pub fn default_zone_opacity() -> f32 {
    0.15
}
