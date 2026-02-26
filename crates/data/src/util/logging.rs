//! Logging Configuration and Utilities

use std::path::{Path, PathBuf};
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

/// Build log file path under the given data directory.
pub fn path_under(data_dir: &Path) -> PathBuf {
    data_dir.join("kairos.log")
}
