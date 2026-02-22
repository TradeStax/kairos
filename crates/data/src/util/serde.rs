//! Serde utility helpers

use serde::{Deserialize, Deserializer};

/// Deserialize a value, falling back to `Default` if parsing fails.
///
/// Used with `#[serde(deserialize_with)]` to gracefully handle
/// configuration changes without breaking saved state files.
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
