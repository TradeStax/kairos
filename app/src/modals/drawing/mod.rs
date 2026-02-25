//! Drawing Tools & Properties
//!
//! Consolidates drawing tool selection (`tools/`) and the drawing
//! properties modal (`properties/`) into a single module.

pub mod tools;
pub mod properties;

// Re-export commonly used types at the drawing:: level
pub use tools::{DrawingToolsPanel, SidebarGroup};
pub use properties::DrawingPropertiesModal;
