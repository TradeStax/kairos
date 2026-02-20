use data::SerializableColor;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Definition of a configurable parameter for a study.
#[derive(Debug, Clone)]
pub struct ParameterDef {
    /// Unique key for this parameter
    pub key: &'static str,
    /// Display label in the UI
    pub label: &'static str,
    /// Tooltip description
    pub description: &'static str,
    /// The kind of parameter (determines UI widget)
    pub kind: ParameterKind,
    /// Default value
    pub default: ParameterValue,
}

/// The kind of a parameter, defining its constraints and UI representation.
#[derive(Debug, Clone)]
pub enum ParameterKind {
    Integer { min: i64, max: i64 },
    Float { min: f64, max: f64, step: f64 },
    Color,
    Boolean,
    Choice { options: &'static [&'static str] },
    LineStyle,
}

/// A concrete parameter value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterValue {
    Integer(i64),
    Float(f64),
    Color(SerializableColor),
    Boolean(bool),
    Choice(String),
    LineStyle(LineStyleValue),
}

/// Line style for study rendering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineStyleValue {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl std::fmt::Display for LineStyleValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LineStyleValue::Solid => write!(f, "Solid"),
            LineStyleValue::Dashed => write!(f, "Dashed"),
            LineStyleValue::Dotted => write!(f, "Dotted"),
        }
    }
}

/// Snapshot of a study's current configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyConfig {
    /// Study identifier
    pub id: String,
    /// Current parameter values keyed by parameter key
    pub values: HashMap<String, ParameterValue>,
}

impl StudyConfig {
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

    /// Set a parameter value.
    pub fn set(&mut self, key: impl Into<String>, value: ParameterValue) {
        self.values.insert(key.into(), value);
    }
}
