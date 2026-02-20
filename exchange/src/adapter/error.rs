//! Adapter-level error type for exchange operations.

use flowsurface_data::domain::error::{AppError, ErrorSeverity};

#[derive(thiserror::Error, Debug)]
pub enum AdapterError {
    #[error("{0}")]
    FetchError(#[from] reqwest::Error),
    #[error("Parsing: {0}")]
    ParseError(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Connection error: {0}")]
    ConnectionError(String),
}

impl AppError for AdapterError {
    fn user_message(&self) -> String {
        match self {
            AdapterError::InvalidRequest(_) => {
                "Invalid request made to the exchange. Check logs for details.".to_string()
            }
            AdapterError::FetchError(_) => {
                "Network error while contacting the exchange.".to_string()
            }
            AdapterError::ParseError(_) => {
                "Unexpected response from the exchange. Check logs for details.".to_string()
            }
            AdapterError::ConnectionError(_) => {
                "Connection error while communicating with the exchange.".to_string()
            }
        }
    }

    fn is_retriable(&self) -> bool {
        matches!(
            self,
            AdapterError::FetchError(_) | AdapterError::ConnectionError(_)
        )
    }

    fn severity(&self) -> ErrorSeverity {
        match self {
            AdapterError::FetchError(_) | AdapterError::ConnectionError(_) => {
                ErrorSeverity::Recoverable
            }
            AdapterError::ParseError(_) => ErrorSeverity::Warning,
            AdapterError::InvalidRequest(_) => ErrorSeverity::Info,
        }
    }
}
