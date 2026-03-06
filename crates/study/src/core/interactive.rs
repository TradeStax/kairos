//! Structured interactive data types replacing `&dyn Any`.
//!
//! These types provide a well-defined interface between studies and the
//! UI layer for interactive features like detail modals, crosshair
//! tooltips, and clickable chart regions.

use data::SerializableColor;

/// Structured payload for interactive study data.
///
/// Replaces the previous `&dyn Any` approach with typed access plus
/// a JSON representation for generic rendering.
pub struct InteractivePayload {
    /// Type identifier (e.g. "level_analyzer").
    pub type_id: &'static str,
    /// Serialized data for generic rendering.
    pub data: serde_json::Value,
    /// Opaque concrete data for type-safe downcast.
    concrete: Box<dyn std::any::Any + Send + Sync>,
}

impl InteractivePayload {
    /// Create a new payload with both JSON and concrete data.
    pub fn new<T: serde::Serialize + std::any::Any + Send + Sync>(
        type_id: &'static str,
        data: &T,
    ) -> Self {
        let json = serde_json::to_value(data).unwrap_or(serde_json::Value::Null);
        Self {
            type_id,
            data: json,
            concrete: Box::new(()),
        }
    }

    /// Create a payload with concrete data for downcasting.
    pub fn with_concrete<T: std::any::Any + Send + Sync>(
        type_id: &'static str,
        json: serde_json::Value,
        concrete: T,
    ) -> Self {
        Self {
            type_id,
            data: json,
            concrete: Box::new(concrete),
        }
    }

    /// Downcast the concrete data to a specific type.
    pub fn downcast_ref<T: std::any::Any>(&self) -> Option<&T> {
        self.concrete.downcast_ref()
    }
}

impl std::fmt::Debug for InteractivePayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InteractivePayload")
            .field("type_id", &self.type_id)
            .finish()
    }
}

/// A clickable/hoverable region on the chart produced by a study.
#[derive(Debug, Clone)]
pub struct InteractiveRegion {
    /// Unique region identifier.
    pub id: u32,
    /// Bounding box: (x_start, y_top, x_end, y_bottom).
    pub bounds: (u64, f64, u64, f64),
    /// What happens when the user interacts with this region.
    pub action: InteractionAction,
    /// Optional tooltip text shown on hover.
    pub tooltip: Option<String>,
}

/// Action triggered by interacting with a chart region.
#[derive(Debug, Clone)]
pub enum InteractionAction {
    /// Open a detail inspector for the given key.
    InspectDetail(String),
    /// Allow dragging to adjust a value.
    DragAdjust { key: String, value: f64 },
    /// No action (hover-only region).
    None,
}

/// A value to display next to the crosshair at a given position.
#[derive(Debug, Clone)]
pub struct CrosshairValue {
    /// Label shown before the value.
    pub label: String,
    /// Formatted value string.
    pub value: String,
    /// Color for the value text.
    pub color: SerializableColor,
}

/// Specification for a study detail modal.
#[derive(Debug, Clone)]
pub struct StudyModalSpec {
    /// Modal title.
    pub title: String,
    /// Sections with key-value rows.
    pub sections: Vec<StudyModalSection>,
}

/// A section within a study detail modal.
#[derive(Debug, Clone)]
pub struct StudyModalSection {
    /// Section heading.
    pub heading: String,
    /// Key-value rows.
    pub rows: Vec<(String, String)>,
}
