use thiserror::Error;

use crate::domain::error::{AppError, ErrorSeverity};
use crate::repository::RepositoryError;
use crate::services::ServiceError;

#[derive(Error, Debug, Clone)]
pub enum DataError {
    #[error("Service error: {0}")]
    Service(String),
    #[error("Repository error: {0}")]
    Repository(String),
    #[error("State error: {0}")]
    State(String),
}

impl AppError for DataError {
    fn user_message(&self) -> String {
        match self {
            Self::Service(s) => format!("Service error: {s}"),
            Self::Repository(s) => format!("Data error: {s}"),
            Self::State(s) => format!("State error: {s}"),
        }
    }

    fn is_retriable(&self) -> bool {
        false
    }

    fn severity(&self) -> ErrorSeverity {
        ErrorSeverity::Recoverable
    }
}

impl From<RepositoryError> for DataError {
    fn from(err: RepositoryError) -> Self {
        DataError::Repository(err.to_string())
    }
}

impl From<ServiceError> for DataError {
    fn from(err: ServiceError) -> Self {
        DataError::Service(err.to_string())
    }
}
