use thiserror::Error;

#[derive(Debug, Error)]
pub enum StudyError {
    #[error("invalid parameter '{key}': {reason}")]
    InvalidParameter { key: String, reason: String },

    #[error("insufficient data: need {needed} candles, have {available}")]
    InsufficientData { needed: usize, available: usize },

    #[error("unknown study: {0}")]
    UnknownStudy(String),
}
