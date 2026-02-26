//! Study-specific error types with severity classification.

use thiserror::Error;

/// Errors produced during study configuration or computation.
#[derive(Debug, Error)]
pub enum StudyError {
    #[error("invalid parameter '{key}': {reason}")]
    InvalidParameter { key: String, reason: String },

    #[error("insufficient data: need {needed} candles, have {available}")]
    InsufficientData { needed: usize, available: usize },

    #[error("unknown study: {0}")]
    UnknownStudy(String),

    #[error("compute error: {0}")]
    Compute(String),
}

impl data::AppError for StudyError {
    fn user_message(&self) -> String {
        self.to_string()
    }

    fn is_retriable(&self) -> bool {
        matches!(self, StudyError::InsufficientData { .. })
    }

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
