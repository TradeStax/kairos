//! Public error hierarchy for the data crate.
//!
//! [`Error`] is the top-level error type exposed to callers. Adapter-internal
//! errors convert into it via `From` impls. Implements the [`AppError`] trait
//! for user-facing messages, retriability, and severity classification.

use crate::domain::error::{AppError, ErrorSeverity};
use thiserror::Error;

/// Top-level error type for the data crate.
///
/// All public API methods return this error. Adapter-specific errors are
/// converted via `From` impls before reaching callers.
#[derive(Error, Debug, Clone)]
pub enum Error {
    /// Data fetch failed (network, timeout, adapter error)
    #[error("Fetch error: {0}")]
    Fetch(String),
    /// Configuration is invalid or missing
    #[error("Config error: {0}")]
    Config(String),
    /// Cache read/write/decode failure
    #[error("Cache error: {0}")]
    Cache(String),
    /// Ticker symbol not found or invalid
    #[error("Symbol error: {0}")]
    Symbol(String),
    /// Connection failed or was lost
    #[error("Connection error: {0}")]
    Connection(String),
    /// Input validation failed
    #[error("Validation error: {0}")]
    Validation(String),
    /// No data available for the requested range
    #[error("No data: {0}")]
    NoData(String),
    /// Trade/candle aggregation error
    #[error("Aggregation error: {0}")]
    Aggregation(#[from] crate::aggregation::AggregationError),
    /// Filesystem I/O error
    #[error("IO error: {0}")]
    Io(String),
}

impl AppError for Error {
    fn user_message(&self) -> String {
        match self {
            Self::Fetch(s) => format!("Data fetch failed: {s}"),
            Self::Config(s) => format!("Configuration error: {s}"),
            Self::Cache(s) => format!("Cache error: {s}"),
            Self::Symbol(s) => format!("Symbol error: {s}"),
            Self::Connection(s) => format!("Connection error: {s}"),
            Self::Validation(s) => format!("Validation error: {s}"),
            Self::NoData(s) => format!("No data available: {s}"),
            Self::Aggregation(e) => format!("Aggregation error: {e}"),
            Self::Io(s) => format!("I/O error: {s}"),
        }
    }

    fn is_retriable(&self) -> bool {
        matches!(self, Self::Fetch(_) | Self::Connection(_))
    }

    fn severity(&self) -> ErrorSeverity {
        match self {
            Self::NoData(_) => ErrorSeverity::Info,
            Self::Fetch(_) | Self::Connection(_) => ErrorSeverity::Warning,
            Self::Config(_) | Self::Validation(_) | Self::Aggregation(_) => {
                ErrorSeverity::Recoverable
            }
            Self::Cache(_) | Self::Symbol(_) | Self::Io(_) => ErrorSeverity::Recoverable,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err.to_string())
    }
}
