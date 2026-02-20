//! Error Types for UI

use data::domain::error::{AppError, ErrorSeverity};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum InternalError {
    #[error("Chart error: {0}")]
    Chart(String),

    #[error("Data error: {0}")]
    Data(String),

    #[error("Rendering error: {0}")]
    Rendering(String),
}

impl AppError for InternalError {
    fn user_message(&self) -> String {
        match self {
            InternalError::Chart(msg) => format!("Chart error: {}", msg),
            InternalError::Data(msg) => format!("Data error: {}", msg),
            InternalError::Rendering(msg) => {
                "A rendering error occurred. Try refreshing the chart.".to_string()
            }
        }
    }

    fn is_retriable(&self) -> bool {
        matches!(self, InternalError::Data(_) | InternalError::Rendering(_))
    }

    fn severity(&self) -> ErrorSeverity {
        match self {
            InternalError::Chart(_) => ErrorSeverity::Warning,
            InternalError::Data(_) => ErrorSeverity::Recoverable,
            InternalError::Rendering(_) => ErrorSeverity::Warning,
        }
    }
}

impl From<String> for InternalError {
    fn from(s: String) -> Self {
        InternalError::Data(s)
    }
}

impl From<&str> for InternalError {
    fn from(s: &str) -> Self {
        InternalError::Data(s.to_string())
    }
}
