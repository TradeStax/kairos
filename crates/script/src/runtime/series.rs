//! Series lookback support via JavaScript.
//!
//! In full-array mode, series are just regular JS arrays. Users access
//! historical values via standard array indexing: `close[barIndex - 1]`.
//!
//! For convenience, the `input.source()` function maps source names
//! to the corresponding global arrays.

/// Source names that map to built-in global arrays.
pub const VALID_SOURCES: &[&str] = &[
    "close", "open", "high", "low", "hl2", "hlc3", "ohlc4", "volume",
];

/// Resolve a source name to the global variable name.
pub fn resolve_source(source: &str) -> Option<&'static str> {
    match source.to_lowercase().as_str() {
        "close" => Some("close"),
        "open" => Some("open"),
        "high" => Some("high"),
        "low" => Some("low"),
        "hl2" => Some("hl2"),
        "hlc3" => Some("hlc3"),
        "ohlc4" => Some("ohlc4"),
        "volume" => Some("volume"),
        _ => None,
    }
}
