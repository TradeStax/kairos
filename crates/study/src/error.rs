//! Study-specific error types with severity classification.
//!
//! [`StudyError`] covers all failure modes encountered during parameter
//! validation, computation, and registry lookup. Each variant carries
//! structured context and maps to an [`ErrorSeverity`](data::ErrorSeverity)
//! level via the [`AppError`](data::AppError) trait, allowing the UI layer
//! to present actionable diagnostics without pattern-matching on internals.

use thiserror::Error;

/// Errors produced during study configuration or computation.
#[derive(Debug, Clone, Error)]
pub enum StudyError {
    /// A parameter value failed validation or the key is unrecognized.
    #[error("invalid parameter '{key}': {reason}")]
    InvalidParameter { key: String, reason: String },

    /// Not enough candle data to satisfy the study's lookback period.
    #[error("insufficient data: need {needed} candles, have {available}")]
    InsufficientData { needed: usize, available: usize },

    /// The requested study ID was not found in the registry.
    #[error("unknown study: {0}")]
    UnknownStudy(String),

    /// A runtime error during the `compute()` pass.
    #[error("compute error: {0}")]
    Compute(String),
}

impl data::AppError for StudyError {
    /// Returns the `thiserror`-formatted message, suitable for display in toasts.
    fn user_message(&self) -> String {
        self.to_string()
    }

    /// Only `InsufficientData` is retriable — more candles may arrive.
    fn is_retriable(&self) -> bool {
        matches!(self, StudyError::InsufficientData { .. })
    }

    /// Maps each variant to a severity level for the notification system.
    fn severity(&self) -> data::ErrorSeverity {
        match self {
            StudyError::InvalidParameter { .. } | StudyError::UnknownStudy(_) => {
                data::ErrorSeverity::Warning
            }
            StudyError::InsufficientData { .. } => data::ErrorSeverity::Info,
            StudyError::Compute(_) => data::ErrorSeverity::Recoverable,
        }
    }
}
