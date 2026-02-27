//! Logging configuration and path utilities.
//!
//! Provides the standard log file path and error types for logger setup.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during logging setup.
#[derive(Error, Debug)]
pub enum Error {
    /// Filesystem I/O error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse a log level string
    #[error("Parse error: {0}")]
    Parse(#[from] log::ParseLevelError),

    /// Failed to set the global logger
    #[error("Set logger error: {0}")]
    SetLogger(#[from] log::SetLoggerError),
}

/// Returns the log file path under the given data directory
#[must_use]
pub fn path_under(data_dir: &Path) -> PathBuf {
    data_dir.join("kairos.log")
}
