//! Compute result feedback from [`Study::compute()`](super::Study::compute).
//!
//! [`StudyResult`] communicates whether the output changed and carries
//! optional diagnostic messages back to the UI layer.

/// Result returned by [`Study::compute()`](super::Study::compute).
///
/// Carries optional diagnostics (info/warning messages) and a flag
/// indicating whether the output actually changed — enabling the
/// renderer to skip cache invalidation when nothing moved.
pub struct StudyResult {
    /// Diagnostic messages produced during computation.
    pub diagnostics: Vec<StudyDiagnostic>,
    /// Whether the study output changed compared to the previous call.
    pub output_changed: bool,
}

/// A diagnostic message produced during study computation.
pub struct StudyDiagnostic {
    /// Severity of the diagnostic.
    pub severity: DiagnosticSeverity,
    /// Human-readable message.
    pub message: String,
}

/// Severity level for [`StudyDiagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    /// Informational — no action needed.
    Info,
    /// Warning — study may produce unexpected results.
    Warning,
}

impl StudyResult {
    /// Computation succeeded and output changed.
    pub fn ok() -> Self {
        Self {
            diagnostics: vec![],
            output_changed: true,
        }
    }

    /// Computation succeeded but output did not change.
    pub fn unchanged() -> Self {
        Self {
            diagnostics: vec![],
            output_changed: false,
        }
    }

    /// Computation succeeded with a warning diagnostic.
    pub fn with_warning(message: impl Into<String>) -> Self {
        Self {
            diagnostics: vec![StudyDiagnostic {
                severity: DiagnosticSeverity::Warning,
                message: message.into(),
            }],
            output_changed: true,
        }
    }

    /// Whether any diagnostics have warning severity.
    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Warning)
    }
}
