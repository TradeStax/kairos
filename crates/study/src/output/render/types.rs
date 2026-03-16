//! Rendering-level types shared across all renderers.

use crate::config::LineStyleValue;

/// Font hint for platform-agnostic text rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontHint {
    /// Monospace font (e.g. for price labels, cluster text).
    Monospace,
    /// Default proportional font.
    Default,
}

/// Rendering-level line dash specification.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LineStyle {
    /// Continuous line.
    #[default]
    Solid,
    /// Dashed segments: `[6.0, 4.0]`.
    Dashed,
    /// Dotted segments: `[2.0, 3.0]`.
    Dotted,
}

impl From<&LineStyleValue> for LineStyle {
    fn from(value: &LineStyleValue) -> Self {
        match value {
            LineStyleValue::Solid => Self::Solid,
            LineStyleValue::Dashed => Self::Dashed,
            LineStyleValue::Dotted => Self::Dotted,
        }
    }
}

/// Horizontal text alignment for canvas text rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    /// Left-aligned (Start).
    Start,
    /// Centered.
    Center,
    /// Right-aligned (End).
    End,
}
