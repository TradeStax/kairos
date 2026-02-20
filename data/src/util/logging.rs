//! Logging Configuration and Utilities

use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] log::ParseLevelError),

    #[error("Set logger error: {0}")]
    SetLogger(#[from] log::SetLoggerError),
}

/// Get log file path
pub fn path() -> Result<PathBuf, Error> {
    let data_dir = crate::data_path(None);
    Ok(data_dir.join("flowsurface.log"))
}
