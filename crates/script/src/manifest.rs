//! ScriptManifest: metadata parsed from indicator() declaration.

use study::traits::StudyCategory;
use std::path::PathBuf;
use std::time::SystemTime;

/// Metadata extracted from an indicator script's `indicator()` declaration
/// and `input.*()` calls during the declaration pass.
#[derive(Debug, Clone)]
pub struct ScriptManifest {
    /// Unique ID derived from filename (e.g., "sma" from "sma.js")
    pub id: String,
    /// Display name from `indicator()` call
    pub name: String,
    /// Overlay on price chart or separate panel
    pub overlay: bool,
    /// Category for UI grouping
    pub category: StudyCategory,
    /// Source file path
    pub path: PathBuf,
    /// File modification time (for hot-reload detection)
    pub modified: SystemTime,
    /// Extracted input declarations
    pub inputs: Vec<InputDeclaration>,
}

/// An input parameter declaration collected from `input.*()` calls.
#[derive(Debug, Clone)]
pub struct InputDeclaration {
    /// Unique key derived from label (slugified)
    pub key: String,
    /// Display label for the UI
    pub label: String,
    /// Description / tooltip
    pub description: String,
    /// Parameter kind with constraints
    pub kind: study::config::ParameterKind,
    /// Default value
    pub default: study::config::ParameterValue,
}

impl InputDeclaration {
    /// Convert to a study ParameterDef (borrows key/label/description).
    ///
    /// NOTE: This creates owned ParameterDef with leaked &'static str references.
    /// In practice, manifests live for the entire app lifetime, so this is acceptable.
    pub fn to_parameter_def(&self) -> study::config::ParameterDef {
        study::config::ParameterDef {
            key: Box::leak(self.key.clone().into_boxed_str()),
            label: Box::leak(self.label.clone().into_boxed_str()),
            description: Box::leak(self.description.clone().into_boxed_str()),
            kind: self.kind.clone(),
            default: self.default.clone(),
        }
    }
}

/// Slugify a label into a parameter key.
///
/// "Line Width" -> "line_width"
/// "Min Contracts" -> "min_contracts"
pub fn slugify(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// Parse a category string into StudyCategory.
pub fn parse_category(s: &str) -> StudyCategory {
    match s.to_lowercase().as_str() {
        "trend" => StudyCategory::Trend,
        "momentum" => StudyCategory::Momentum,
        "volume" => StudyCategory::Volume,
        "volatility" => StudyCategory::Volatility,
        "orderflow" | "order_flow" => StudyCategory::OrderFlow,
        _ => StudyCategory::Custom,
    }
}
