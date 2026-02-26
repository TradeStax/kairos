use super::Content;

impl Content {
    /// Serialize drawings for persistence
    pub fn serialize_drawings(&self) -> Vec<crate::drawing::SerializableDrawing> {
        self.drawing_chart()
            .map(|c| c.drawings().to_serializable())
            .unwrap_or_default()
    }
}
