//! Comprehensive error handling for FlowSurface Exchange layer
//!
//! Provides unified error types with proper context and user-friendly messages.

use flowsurface_data::domain::error::{AppError, ErrorSeverity};

/// Result type alias for exchange operations
pub type ExchangeResult<T> = std::result::Result<T, Error>;

/// Comprehensive error type for all exchange operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Data fetching errors (API, network, etc.)
    #[error("Data fetch failed: {0}")]
    Fetch(String),

    /// Data parsing errors (malformed responses, schema mismatches)
    #[error("Parse error: {0}")]
    Parse(String),

    /// Configuration errors (missing API keys, invalid settings)
    #[error("Configuration error: {0}")]
    Config(String),

    /// Cache errors (I/O failures, corruption)
    #[error("Cache error: {0}")]
    Cache(String),

    /// Symbol/ticker errors (not found, invalid format)
    #[error("Symbol error: {0}")]
    Symbol(String),

    /// Data validation errors (missing fields, out of range)
    #[error("Validation error: {0}")]
    Validation(String),

    /// Databento-specific errors
    #[error("Databento error: {0}")]
    Databento(#[from] databento::Error),

    /// DBN format errors
    #[error("DBN format error: {0}")]
    Dbn(#[from] databento::dbn::Error),

    /// Rithmic errors
    #[error("Rithmic error: {0}")]
    Rithmic(#[from] crate::adapter::rithmic::RithmicError),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

// Manual From implementation for DatabentoError (can't use #[from] due to module visibility)
impl From<crate::adapter::databento::DatabentoError> for Error {
    fn from(err: crate::adapter::databento::DatabentoError) -> Self {
        use crate::adapter::databento::DatabentoError;
        match err {
            DatabentoError::Api(e) => Error::Databento(e),
            DatabentoError::Dbn(e) => Error::Dbn(e),
            DatabentoError::SymbolNotFound(s) => Error::Symbol(s),
            DatabentoError::InvalidInstrumentId(id) => {
                Error::Symbol(format!("Invalid instrument ID: {}", id))
            }
            DatabentoError::Cache(s) => Error::Cache(s),
            DatabentoError::Config(s) => Error::Config(s),
        }
    }
}

impl AppError for Error {
    fn user_message(&self) -> String {
        match self {
            Error::Fetch(msg) => format!("Failed to fetch data: {}", msg),
            Error::Parse(msg) => format!("Failed to parse response: {}", msg),
            Error::Config(msg) => format!("Configuration error: {}", msg),
            Error::Cache(msg) => format!("Cache error: {}", msg),
            Error::Symbol(msg) => format!("Symbol error: {}", msg),
            Error::Validation(msg) => format!("Invalid data: {}", msg),
            Error::Databento(_) => {
                "Databento API error - check connectivity and API key".to_string()
            }
            Error::Dbn(_) => "Data format error - corrupted or incompatible data".to_string(),
            Error::Io(_) => "File system error - check permissions and disk space".to_string(),
            Error::Rithmic(_) => {
                "Rithmic API error - check connectivity and credentials".to_string()
            }
        }
    }

    fn is_retriable(&self) -> bool {
        matches!(
            self,
            Error::Fetch(_) | Error::Databento(_) | Error::Io(_) | Error::Rithmic(_)
        )
    }

    fn severity(&self) -> ErrorSeverity {
        match self {
            Error::Config(_) | Error::Dbn(_) => ErrorSeverity::Critical,
            Error::Fetch(_) | Error::Databento(_) | Error::Io(_) | Error::Rithmic(_) => {
                ErrorSeverity::Recoverable
            }
            Error::Parse(_) | Error::Cache(_) => ErrorSeverity::Warning,
            Error::Symbol(_) | Error::Validation(_) => ErrorSeverity::Info,
        }
    }
}

/// Helper macro for creating errors with context
#[macro_export]
macro_rules! error {
    (fetch: $($arg:tt)*) => {
        $crate::error::Error::Fetch(format!($($arg)*))
    };
    (parse: $($arg:tt)*) => {
        $crate::error::Error::Parse(format!($($arg)*))
    };
    (config: $($arg:tt)*) => {
        $crate::error::Error::Config(format!($($arg)*))
    };
    (cache: $($arg:tt)*) => {
        $crate::error::Error::Cache(format!($($arg)*))
    };
    (symbol: $($arg:tt)*) => {
        $crate::error::Error::Symbol(format!($($arg)*))
    };
    (validation: $($arg:tt)*) => {
        $crate::error::Error::Validation(format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_messages() {
        let err = Error::Fetch("network timeout".to_string());
        assert!(err.user_message().contains("Failed to fetch"));
        assert!(err.is_retriable());
        assert_eq!(err.severity(), ErrorSeverity::Recoverable);

        let err = Error::Config("missing API key".to_string());
        assert!(!err.is_retriable());
        assert_eq!(err.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_error_macro() {
        let err = error!(fetch: "timeout after {}ms", 5000);
        assert!(matches!(err, Error::Fetch(_)));
    }
}
