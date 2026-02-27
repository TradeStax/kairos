//! Parameter definitions: kinds, tabs, sections, and validation.
//!
//! Each study declares its configurable parameters as a `&[ParameterDef]`
//! slice. A [`ParameterDef`] carries the key, label, type constraints
//! ([`ParameterKind`]), default value, tab/section placement, and
//! conditional visibility. The settings UI is generated entirely from
//! these definitions — no manual widget wiring is needed.

use serde::{Deserialize, Serialize};

use super::display::{DisplayFormat, Visibility};
use super::value::ParameterValue;

/// Settings tab that a parameter belongs to in the study settings modal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParameterTab {
    /// Core numeric parameters (period, multiplier, threshold).
    #[default]
    Parameters,
    /// Visual styling (colors, line styles, widths).
    Style,
    /// Display toggles and formatting options.
    Display,
    /// Point-of-control settings (Volume Profile studies).
    PocSettings,
    /// Value area configuration (Volume Profile studies).
    ValueArea,
    /// High/low volume node detection (Volume Profile studies).
    Nodes,
    /// Session VWAP bands and anchoring (VWAP studies).
    Vwap,
    /// Absorption detection parameters (Big Trades study).
    Absorption,
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
            ParameterTab::Absorption => write!(f, "Absorption"),
        }
    }
}

/// Named section within a settings tab for grouping related parameters.
///
/// Sections provide visual separation (e.g. "Line", "Fill", "Labels"
/// within a Style tab). Parameters with the same section are rendered
/// together under a shared heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterSection {
    /// Section heading displayed in the UI.
    pub label: &'static str,
    /// Sort order among sections in the same tab (lower = earlier).
    pub order: u16,
}

/// Full definition of a configurable study parameter.
///
/// Drives the settings UI: widget type, validation, tab placement, and
/// conditional visibility are all derived from this definition.
#[derive(Debug, Clone)]
pub struct ParameterDef {
    /// Unique key used to store and retrieve the value in [`super::StudyConfig`].
    pub key: String,
    /// Human-readable label shown next to the widget.
    pub label: String,
    /// Tooltip or help text describing the parameter's effect.
    pub description: String,
    /// Type and constraints — determines which widget is rendered.
    pub kind: ParameterKind,
    /// Initial value applied when a study is first created.
    pub default: ParameterValue,
    /// Settings tab this parameter appears in.
    pub tab: ParameterTab,
    /// Optional section grouping within the tab.
    pub section: Option<ParameterSection>,
    /// Sort order within the section/tab (lower = earlier).
    pub order: u16,
    /// How the value is formatted for display in labels and sliders.
    pub format: DisplayFormat,
    /// Conditional visibility rule — hides this parameter when irrelevant.
    pub visible_when: Visibility,
}

/// The kind of a parameter — determines the UI widget and value constraints.
#[derive(Debug, Clone)]
pub enum ParameterKind {
    /// Whole number with inclusive bounds. Renders as a slider or numeric input.
    Integer { min: i64, max: i64 },
    /// Decimal number with inclusive bounds and step increment.
    Float { min: f64, max: f64, step: f64 },
    /// RGBA color. Renders as a color picker.
    Color,
    /// On/off toggle. Renders as a checkbox.
    Boolean,
    /// Selection from a fixed set of string options. Renders as a dropdown.
    Choice { options: &'static [&'static str] },
    /// Line rendering style (solid, dashed, dotted). Renders as a dropdown.
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
                    return Err(format!("invalid {}: {}", self.label, s));
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
