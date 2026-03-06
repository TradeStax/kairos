//! Cross-study composition types.
//!
//! [`CompositeStudy`](super::capabilities::CompositeStudy) declares
//! dependencies on other studies and receives their resolved outputs
//! via [`DependencyOutputs`] during computation.

use std::collections::HashMap;

use crate::config::ParameterValue;
use crate::output::StudyOutput;

/// A dependency on another study, identified by study ID and alias.
#[derive(Debug, Clone)]
pub struct StudyDependency {
    /// Study ID to depend on (e.g. "sma").
    pub study_id: &'static str,
    /// Local alias for accessing the dependency's output.
    pub alias: &'static str,
    /// Parameter overrides for the dependency instance.
    pub params: Vec<(&'static str, ParameterValue)>,
}

/// Resolved outputs from dependency studies.
///
/// Passed to [`CompositeStudy::compute_with_deps()`](super::capabilities::CompositeStudy::compute_with_deps).
pub struct DependencyOutputs {
    outputs: HashMap<String, StudyOutput>,
}

impl DependencyOutputs {
    /// Create from a map of alias → output.
    pub fn new(outputs: HashMap<String, StudyOutput>) -> Self {
        Self { outputs }
    }

    /// Get a dependency's output by alias.
    pub fn get(&self, alias: &str) -> Option<&StudyOutput> {
        self.outputs.get(alias)
    }

    /// Get line series points from a dependency by alias and series index.
    pub fn get_line_points(&self, alias: &str, series_index: usize) -> Option<&[(u64, f32)]> {
        match self.outputs.get(alias)? {
            StudyOutput::Lines(lines) => lines.get(series_index).map(|s| s.points.as_slice()),
            _ => None,
        }
    }
}
