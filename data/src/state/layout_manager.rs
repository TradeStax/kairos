//! Layout Manager Types for State Serialization

use serde::{Deserialize, Serialize};
use super::layout_types::Dashboard;

/// Layout definition for state persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    pub dashboard: Dashboard,
}

/// Layout manager state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutManager {
    pub layouts: Vec<Layout>,
    pub active_layout: Option<String>,  // Name of active layout
}

impl LayoutManager {
    pub fn new(layout_name: String, dashboard: Dashboard) -> Self {
        Self {
            layouts: vec![Layout {
                name: layout_name.clone(),
                dashboard,
            }],
            active_layout: Some(layout_name),
        }
    }
}

/// List of layouts (for serialization)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layouts {
    pub items: Vec<Layout>,
    pub current_index: usize,
}

impl Default for Layouts {
    fn default() -> Self {
        Self {
            items: vec![],
            current_index: 0,
        }
    }
}
