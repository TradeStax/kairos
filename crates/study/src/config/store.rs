//! Runtime parameter storage with typed getters.
//!
//! [`StudyConfig`] holds the current values for all parameters of a study
//! instance. It is serialized alongside pane state so that study settings
//! persist across sessions. Typed getters return a caller-supplied default
//! when a key is absent or the stored type does not match.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use data::SerializableColor;

use super::parameter::ParameterDef;
use super::value::{LineStyleValue, ParameterValue};
use crate::error::StudyError;

/// Runtime snapshot of a study's current parameter values.
///
/// Persisted alongside pane state so studies restore their settings across
/// sessions. Values are keyed by parameter name and accessed via typed getters
/// that return a default when the key is missing or the type doesn't match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyConfig {
    /// Study identifier matching [`super::super::core::Study::id()`].
    pub id: String,
    /// Current parameter values keyed by parameter name.
    pub values: HashMap<String, ParameterValue>,
}

impl StudyConfig {
    /// Create an empty config for the given study ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            values: HashMap::new(),
        }
    }

    /// Get a parameter value by key.
    pub fn get(&self, key: &str) -> Option<&ParameterValue> {
        self.values.get(key)
    }

    /// Get an integer parameter, returning the default if missing or wrong type.
    pub fn get_int(&self, key: &str, default: i64) -> i64 {
        match self.values.get(key) {
            Some(ParameterValue::Integer(v)) => *v,
            _ => default,
        }
    }

    /// Get a float parameter, returning the default if missing or wrong type.
    pub fn get_float(&self, key: &str, default: f64) -> f64 {
        match self.values.get(key) {
            Some(ParameterValue::Float(v)) => *v,
            _ => default,
        }
    }

    /// Get a color parameter, returning the default if missing or wrong type.
    pub fn get_color(&self, key: &str, default: SerializableColor) -> SerializableColor {
        match self.values.get(key) {
            Some(ParameterValue::Color(v)) => *v,
            _ => default,
        }
    }

    /// Get a boolean parameter, returning the default if missing or wrong type.
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        match self.values.get(key) {
            Some(ParameterValue::Boolean(v)) => *v,
            _ => default,
        }
    }

    /// Get a choice parameter, returning the default if missing or wrong type.
    pub fn get_choice<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        match self.values.get(key) {
            Some(ParameterValue::Choice(v)) => v.as_str(),
            _ => default,
        }
    }

    /// Get a line style parameter, returning the default if missing or wrong type.
    pub fn get_line_style(&self, key: &str, default: LineStyleValue) -> LineStyleValue {
        match self.values.get(key) {
            Some(ParameterValue::LineStyle(v)) => *v,
            _ => default,
        }
    }

    /// Set a parameter value (no validation).
    pub fn set(&mut self, key: impl Into<String>, value: ParameterValue) {
        self.values.insert(key.into(), value);
    }

    /// Validate a value against the parameter definition, then set it.
    pub fn validate_and_set(
        &mut self,
        key: &str,
        value: ParameterValue,
        params: &[ParameterDef],
    ) -> Result<(), StudyError> {
        let def =
            params
                .iter()
                .find(|p| p.key == key)
                .ok_or_else(|| StudyError::InvalidParameter {
                    key: key.to_string(),
                    reason: "unknown parameter".to_string(),
                })?;

        def.validate(&value)
            .map_err(|reason| StudyError::InvalidParameter {
                key: key.to_string(),
                reason,
            })?;

        self.set(key, value);
        Ok(())
    }
}
