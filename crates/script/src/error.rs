use thiserror::Error;

/// Errors that can occur in the script engine.
#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("parse error in '{file}': {message}")]
    Parse { file: String, message: String },

    #[error("runtime error in '{file}': {message}")]
    Runtime { file: String, message: String },

    #[error("script '{file}' exceeded {timeout_ms}ms execution time limit")]
    Timeout { file: String, timeout_ms: u64 },

    #[error("script '{file}' exceeded {limit_mb}MB memory limit")]
    Memory { file: String, limit_mb: usize },

    #[error("invalid output from '{file}': {message}")]
    InvalidOutput { file: String, message: String },

    #[error("missing indicator() declaration in '{file}'")]
    MissingDeclaration { file: String },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("QuickJS error: {0}")]
    QuickJs(String),
}

impl ScriptError {
    pub fn user_message(&self) -> String {
        self.to_string()
    }

    pub fn is_retriable(&self) -> bool {
        false
    }

    pub fn severity(&self) -> data::ErrorSeverity {
        match self {
            ScriptError::Parse { .. } | ScriptError::InvalidOutput { .. } => {
                data::ErrorSeverity::Warning
            }
            ScriptError::Timeout { .. } | ScriptError::Memory { .. } => {
                data::ErrorSeverity::Recoverable
            }
            _ => data::ErrorSeverity::Warning,
        }
    }
}

impl From<rquickjs::Error> for ScriptError {
    fn from(err: rquickjs::Error) -> Self {
        ScriptError::QuickJs(err.to_string())
    }
}
