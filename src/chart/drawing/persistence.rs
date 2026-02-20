//! Drawing Persistence
//!
//! Save and load drawings to/from disk, keyed by ticker and chart basis.

use data::SerializableDrawing;
use std::path::PathBuf;

/// Get the drawings file path for a chart
pub fn drawings_path(ticker: &str, basis: &str) -> PathBuf {
    let mut path = crate::infra::platform::data_path(Some("drawings"));
    path.push(format!("{}_{}.json", ticker, basis));
    path
}

/// Save drawings to disk
pub fn save_drawings(
    ticker: &str,
    basis: &str,
    drawings: &[SerializableDrawing],
) -> Result<(), std::io::Error> {
    let path = drawings_path(ticker, basis);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(drawings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, json)
}

/// Load drawings from disk
pub fn load_drawings(ticker: &str, basis: &str) -> Vec<SerializableDrawing> {
    let path = drawings_path(ticker, basis);
    match std::fs::read_to_string(path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}
