use crate::error::StudyError;
use data::SerializableColor;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── UI metadata types for data-driven settings ─────────────────────

/// Which settings tab a parameter appears in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParameterTab {
    Parameters,
    Style,
    Display,
    Tab4,
    Tab5,
    Tab6,
    Tab7,
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
            ParameterTab::Tab4 => write!(f, "Tab 4"),
            ParameterTab::Tab5 => write!(f, "Tab 5"),
            ParameterTab::Tab6 => write!(f, "Tab 6"),
            ParameterTab::Tab7 => write!(f, "Tab 7"),
        }
    }
}

/// Grouping section within a settings tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterSection {
    pub label: &'static str,
    pub order: u16,
}

/// How to format a parameter's value in the UI (e.g. slider labels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayFormat {
    /// Automatic formatting based on ParameterKind
    Auto,
    /// Integer with optional suffix (e.g. "14 bars")
    Integer { suffix: &'static str },
    /// Float with specified decimal places
    Float { decimals: u8 },
    /// Display as percentage
    Percent,
    /// Integer where a specific value means "None/Auto"
    IntegerOrNone { none_value: i64 },
}

impl Default for DisplayFormat {
    fn default() -> Self {
        Self::Auto
    }
}

/// Conditional visibility for a parameter in the settings UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// Always visible
    Always,
    /// Visible when another Choice parameter equals a specific value
    WhenChoice {
        key: &'static str,
        equals: &'static str,
    },
    /// Visible when another Choice parameter does NOT equal a value
    WhenNotChoice {
        key: &'static str,
        not_equals: &'static str,
    },
    /// Visible when another Boolean parameter is true
    WhenTrue(&'static str),
    /// Visible when another Boolean parameter is false
    WhenFalse(&'static str),
}

impl Default for Visibility {
    fn default() -> Self {
        Self::Always
    }
}

impl Visibility {
    /// Evaluate whether this parameter should be visible given the
    /// current configuration values.
    pub fn is_visible(&self, config: &StudyConfig) -> bool {
        match self {
            Visibility::Always => true,
            Visibility::WhenChoice { key, equals } => {
                config.get_choice(key, "") == *equals
            }
            Visibility::WhenNotChoice { key, not_equals } => {
                config.get_choice(key, "") != *not_equals
            }
            Visibility::WhenTrue(key) => config.get_bool(key, false),
            Visibility::WhenFalse(key) => !config.get_bool(key, true),
        }
    }
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
                if *v < *min || *v > *max {
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
        let def = params
            .iter()
            .find(|p| p.key == key)
            .ok_or_else(|| StudyError::InvalidParameter {
                key: key.to_string(),
                reason: "unknown parameter".to_string(),
            })?;

        def.validate(&value).map_err(|reason| {
            StudyError::InvalidParameter {
                key: key.to_string(),
                reason,
            }
        })?;

        self.set(key, value);
        Ok(())
    }
}
