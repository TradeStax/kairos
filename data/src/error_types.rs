//! Error Types for UI

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
