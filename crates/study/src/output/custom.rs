//! Custom output trait for extensible study rendering.
//!
//! Studies can return [`StudyOutput::Custom`](super::StudyOutput::Custom)
//! with a boxed [`CustomOutput`] implementor to provide rendering data
//! that doesn't fit the built-in output variants.

/// Trait for custom study output types.
///
/// Implement this for study outputs that need rendering beyond the
/// standard line/bar/histogram/profile/footprint primitives.
pub trait CustomOutput: Send + Sync + std::fmt::Debug {
    /// Type identifier for renderer dispatch (e.g. "heatmap_grid").
    fn output_type(&self) -> &str;

    /// Optional Y-axis value range for autoscaling.
    fn value_range(&self) -> Option<(f32, f32)>;

    /// Serialize to JSON for AI/snapshot access.
    fn to_json(&self) -> serde_json::Value;

    /// Clone into a new boxed instance.
    fn clone_custom(&self) -> Box<dyn CustomOutput>;

    /// Downcast support.
    fn as_any(&self) -> &dyn std::any::Any;
}

impl Clone for Box<dyn CustomOutput> {
    fn clone(&self) -> Self {
        self.clone_custom()
    }
}
