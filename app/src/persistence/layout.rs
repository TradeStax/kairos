//! Layout Types and Management
//!
//! Types for serializing and managing dashboard layouts, including
//! pane tree structures, split configuration, and layout persistence.

use super::app_state::WindowSpec;
use crate::screen::dashboard::pane::config::{ContentKind, LinkGroup, Settings};
use serde::{Deserialize, Serialize};

/// Pane split axis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Pane tree structure for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Pane {
    Split {
        axis: Axis,
        ratio: f32,
        a: Box<Pane>,
        b: Box<Pane>,
    },
    Content {
        kind: ContentKind,
        settings: Box<Settings>,
        link_group: Option<LinkGroup>,
    },
}

impl Default for Pane {
    fn default() -> Self {
        Pane::Content {
            kind: ContentKind::Starter,
            settings: Box::new(Settings::default()),
            link_group: None,
        }
    }
}

/// Dashboard layout for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dashboard {
    pub pane: Pane,
    pub popout: Vec<(Pane, WindowSpec)>,
}

/// Layout definition for state persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    pub dashboard: Dashboard,
}

/// Layout manager state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayoutManager {
    pub layouts: Vec<Layout>,
    /// Name of active layout
    pub active_layout: Option<String>,
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Layouts {
    pub items: Vec<Layout>,
    pub current_index: usize,
}
