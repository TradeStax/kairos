//! Unified error hierarchy for the data layer.
//!
//! Defines [`ErrorSeverity`] for categorizing errors and the [`AppError`]
//! trait that all data-layer error types implement for consistent handling.

use std::fmt;

/// Severity level for categorizing errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorSeverity {
    /// Informational — not a problem (e.g. data not found)
    Info,
    /// Degraded but functional (e.g. rate limit, temporary network issue)
    Warning,
    /// Operation failed but application is stable
    Recoverable,
    /// System-level failure requiring attention
    Critical,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARNING"),
            Self::Recoverable => write!(f, "ERROR"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Trait for production error types in the data layer.
///
/// All error types implement this trait to provide consistent error
/// handling for the application layer: a UI-safe message, retryability
/// flag, and severity classification.
pub trait AppError: std::error::Error {
    /// Human-readable message safe for display in the UI
    fn user_message(&self) -> String;

    /// Whether the failed operation can be retried
    fn is_retriable(&self) -> bool;

    /// Severity level for logging and alerting
    fn severity(&self) -> ErrorSeverity;
}
