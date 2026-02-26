//! Serde utility helpers and default value functions

use serde::{Deserialize, Deserializer};

/// Deserialize a value, falling back to `Default` if parsing fails.
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

// Default value functions for serde

pub fn default_true() -> bool {
    true
}

pub fn default_one() -> i64 {
    1
}

pub fn default_split_value() -> i64 {
    1
}

pub fn default_max_profiles() -> i64 {
    20
}

pub fn default_va_pct() -> f32 {
    0.7
}

pub fn default_poc_width() -> f32 {
    1.5
}

pub fn default_hvn_threshold() -> f32 {
    0.85
}

pub fn default_lvn_threshold() -> f32 {
    0.15
}

pub fn default_opacity() -> f32 {
    0.7
}

pub fn default_va_fill_opacity() -> f32 {
    0.08
}

pub fn default_line_width() -> f32 {
    1.0
}

pub fn default_zone_opacity() -> f32 {
    0.15
}
