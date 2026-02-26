//! Public Error Hierarchy
//!
//! Two-level error design: public `Error` for callers, adapter-internal
//! errors convert into it via `From` impls.

use crate::domain::error::{AppError, ErrorSeverity};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error("Fetch error: {0}")]
    Fetch(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("Cache error: {0}")]
    Cache(String),
    #[error("Symbol error: {0}")]
    Symbol(String),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("No data: {0}")]
    NoData(String),
    #[error("Aggregation error: {0}")]
    Aggregation(#[from] crate::aggregation::AggregationError),
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
