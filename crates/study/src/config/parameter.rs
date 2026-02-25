use super::display::{DisplayFormat, Visibility};
use super::value::ParameterValue;
use serde::{Deserialize, Serialize};

/// Which settings tab a parameter appears in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParameterTab {
    Parameters,
    Style,
    Display,
    PocSettings,
    ValueArea,
    Nodes,
    Vwap,
}

impl Default for ParameterTab {
    fn default() -> Self {
        Self::Parameters
    }
}

impl std::fmt::Display for ParameterTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParameterTab::Parameters => write!(f, "Parameters"),
            ParameterTab::Style => write!(f, "Style"),
            ParameterTab::Display => write!(f, "Display"),
            ParameterTab::PocSettings => write!(f, "POC"),
            ParameterTab::ValueArea => write!(f, "Value Area"),
            ParameterTab::Nodes => write!(f, "Nodes"),
            ParameterTab::Vwap => write!(f, "VWAP"),
        }
    }
}

/// Grouping section within a settings tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterSection {
    pub label: &'static str,
    pub order: u16,
}

/// Definition of a configurable parameter for a study.
#[derive(Debug, Clone)]
pub struct ParameterDef {
    /// Unique key for this parameter
    pub key: String,
    /// Display label in the UI
    pub label: String,
    /// Tooltip description
    pub description: String,
    /// The kind of parameter (determines UI widget)
    pub kind: ParameterKind,
    /// Default value
    pub default: ParameterValue,
    /// Which settings tab this parameter belongs to
    pub tab: ParameterTab,
    /// Optional grouping section within the tab
    pub section: Option<ParameterSection>,
    /// Sort order within the section/tab (lower = higher)
    pub order: u16,
    /// How to format the value for display
    pub format: DisplayFormat,
    /// Conditional visibility based on other parameter values
    pub visible_when: Visibility,
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

impl ParameterDef {
    /// Validate a value against this parameter's kind and constraints.
    pub fn validate(&self, value: &ParameterValue) -> Result<(), String> {
        match (&self.kind, value) {
            (ParameterKind::Integer { min, max }, ParameterValue::Integer(v)) => {
                if *v < *min || *v > *max {
                    return Err(format!(
                        "{} must be between {} and {}",
                        self.label, min, max
                    ));
                }
            }
            (ParameterKind::Float { min, max, .. }, ParameterValue::Float(v)) => {
                if !v.is_finite() || *v < *min || *v > *max {
                    return Err(format!(
                        "{} must be between {} and {}",
                        self.label, min, max
                    ));
                }
            }
            (ParameterKind::Color, ParameterValue::Color(_)) => {}
            (ParameterKind::Boolean, ParameterValue::Boolean(_)) => {}
            (ParameterKind::Choice { options }, ParameterValue::Choice(s)) => {
                if !options.contains(&s.as_str()) {
                    return Err(format!(
                        "invalid {}: {}",
                        self.label, s
                    ));
                }
            }
            (ParameterKind::LineStyle, ParameterValue::LineStyle(_)) => {}
            _ => {
                return Err(format!(
                    "expected {} for {}",
                    self.kind.type_name(),
                    self.label,
                ));
            }
        }
        Ok(())
    }
}

impl ParameterKind {
    /// Human-readable type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            ParameterKind::Integer { .. } => "integer",
            ParameterKind::Float { .. } => "float",
            ParameterKind::Color => "color",
            ParameterKind::Boolean => "boolean",
            ParameterKind::Choice { .. } => "choice",
            ParameterKind::LineStyle => "line style",
        }
    }
}
