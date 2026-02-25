use super::study_config::StudyConfig;

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
            Visibility::WhenFalse(key) => !config.get_bool(key, false),
        }
    }
}
