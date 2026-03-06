//! Concrete parameter value types and line style enum.
//!
//! [`ParameterValue`] is the runtime representation of a study setting.
//! [`LineStyleValue`] is one of its specialized variants, controlling
//! how study lines are rendered on the chart canvas.

use data::SerializableColor;
use serde::{Deserialize, Serialize};

/// A concrete parameter value stored in [`super::StudyConfig`].
///
/// Each variant corresponds to a [`super::ParameterKind`] and is validated
/// against the constraints defined in the study's [`super::ParameterDef`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterValue {
    /// Whole number (period length, count, offset).
    Integer(i64),
    /// Decimal number (multiplier, threshold, step size).
    Float(f64),
    /// RGBA color for lines, fills, or markers.
    Color(SerializableColor),
    /// On/off toggle (show bands, enable fill, etc.).
    Boolean(bool),
    /// Selection from a fixed set of string options.
    Choice(String),
    /// Visual style for rendered lines.
    LineStyle(LineStyleValue),
    /// Multiple selections from a set of options.
    MultiChoice(Vec<String>),
}

/// Line rendering style for study overlays and panel series.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineStyleValue {
    /// Continuous unbroken line.
    #[default]
    Solid,
    /// Alternating drawn and blank segments.
    Dashed,
    /// Series of small dots.
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
