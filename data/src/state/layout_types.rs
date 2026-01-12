//! Layout Types for Serialization

use serde::{Deserialize, Serialize};
use super::pane_config::{ContentKind, LinkGroup, Settings, VisualConfig};

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
        settings: Settings,
        link_group: Option<LinkGroup>,
    },
}

impl Default for Pane {
    fn default() -> Self {
        Pane::Content {
            kind: ContentKind::Starter,
            settings: Settings::default(),
            link_group: None,
        }
    }
}

/// Dashboard layout for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dashboard {
    pub pane: Pane,
    pub popout: Vec<(Pane, super::WindowSpec)>,
}
