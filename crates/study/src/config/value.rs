//! Concrete parameter value types and line style enum.

use data::SerializableColor;
use serde::{Deserialize, Serialize};

/// A concrete parameter value stored in [`super::StudyConfig`].
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
