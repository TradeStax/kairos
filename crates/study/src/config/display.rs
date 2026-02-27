//! Display formatting and conditional visibility rules for the settings UI.
//!
//! [`DisplayFormat`] controls how parameter values are rendered in slider
//! labels and input fields.  [`Visibility`] drives conditional show/hide
//! logic so that dependent parameters only appear when relevant.

use super::store::StudyConfig;

/// How to format a parameter value for display in slider labels and input fields.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DisplayFormat {
    /// Let the UI choose formatting based on the parameter kind.
    #[default]
    Auto,
    /// Integer with optional unit suffix (e.g. "14 bars").
    Integer { suffix: &'static str },
    /// Float rounded to `decimals` places.
    Float { decimals: u8 },
    /// Multiply by 100 and append "%" (e.g. 0.02 displays as "2%").
    Percent,
    /// Shows "None" or "Auto" when the value equals `none_value`.
    IntegerOrNone { none_value: i64 },
}

/// Conditional visibility for a parameter in the settings UI.
///
/// Used to hide/show parameters based on the current value of other parameters
/// (e.g. show band width only when bands are enabled).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    /// Parameter is always shown.
    #[default]
    Always,
    /// Shown only when a `Choice` parameter equals a specific value.
    WhenChoice {
        key: &'static str,
        equals: &'static str,
    },
    /// Shown only when a `Choice` parameter does not equal a specific value.
    WhenNotChoice {
        key: &'static str,
        not_equals: &'static str,
    },
    /// Shown only when a `Boolean` parameter is `true`.
    WhenTrue(&'static str),
    /// Shown only when a `Boolean` parameter is `false`.
    WhenFalse(&'static str),
}

impl Visibility {
    /// Returns `true` if this parameter should be visible given the current config.
    pub fn is_visible(&self, config: &StudyConfig) -> bool {
        match self {
            Visibility::Always => true,
            Visibility::WhenChoice { key, equals } => config.get_choice(key, "") == *equals,
            Visibility::WhenNotChoice { key, not_equals } => {
                config.get_choice(key, "") != *not_equals
            }
            Visibility::WhenTrue(key) => config.get_bool(key, false),
            Visibility::WhenFalse(key) => !config.get_bool(key, false),
        }
    }
}
