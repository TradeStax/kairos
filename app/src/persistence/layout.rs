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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_starter_pane() -> Pane {
        Pane::Content {
            kind: ContentKind::Starter,
            settings: Box::new(Settings::default()),
            link_group: None,
        }
    }

    fn make_candlestick_pane() -> Pane {
        Pane::Content {
            kind: ContentKind::CandlestickChart,
            settings: Box::new(Settings::default()),
            link_group: None,
        }
    }

    // ── Pane ────────────────────────────────────────────────

    #[test]
    fn pane_default_is_starter() {
        let pane = Pane::default();
        match pane {
            Pane::Content { kind, .. } => assert_eq!(kind, ContentKind::Starter),
            _ => panic!("default pane should be Content"),
        }
    }

    #[test]
    fn pane_split_serialization_roundtrip() {
        let pane = Pane::Split {
            axis: Axis::Horizontal,
            ratio: 0.5,
            a: Box::new(make_starter_pane()),
            b: Box::new(make_candlestick_pane()),
        };
        let json = serde_json::to_string(&pane).unwrap();
        let loaded: Pane = serde_json::from_str(&json).unwrap();
        match loaded {
            Pane::Split { axis, ratio, .. } => {
                assert_eq!(axis, Axis::Horizontal);
                assert!((ratio - 0.5).abs() < 0.01);
            }
            _ => panic!("expected Split"),
        }
    }

    #[test]
    fn pane_content_serialization_roundtrip() {
        let pane = make_candlestick_pane();
        let json = serde_json::to_string(&pane).unwrap();
        let loaded: Pane = serde_json::from_str(&json).unwrap();
        match loaded {
            Pane::Content { kind, .. } => {
                assert_eq!(kind, ContentKind::CandlestickChart);
            }
            _ => panic!("expected Content"),
        }
    }

    #[test]
    fn pane_nested_split_serialization() {
        let pane = Pane::Split {
            axis: Axis::Vertical,
            ratio: 0.3,
            a: Box::new(Pane::Split {
                axis: Axis::Horizontal,
                ratio: 0.6,
                a: Box::new(make_starter_pane()),
                b: Box::new(make_candlestick_pane()),
            }),
            b: Box::new(make_starter_pane()),
        };
        let json = serde_json::to_string(&pane).unwrap();
        let loaded: Pane = serde_json::from_str(&json).unwrap();
        match loaded {
            Pane::Split { axis, ratio, a, .. } => {
                assert_eq!(axis, Axis::Vertical);
                assert!((ratio - 0.3).abs() < 0.01);
                match *a {
                    Pane::Split { axis, .. } => assert_eq!(axis, Axis::Horizontal),
                    _ => panic!("expected nested Split"),
                }
            }
            _ => panic!("expected Split"),
        }
    }

    // ── Axis ────────────────────────────────────────────────

    #[test]
    fn axis_serialization_roundtrip() {
        let h: Axis = serde_json::from_str("\"Horizontal\"").unwrap();
        assert_eq!(h, Axis::Horizontal);
        let v: Axis = serde_json::from_str("\"Vertical\"").unwrap();
        assert_eq!(v, Axis::Vertical);
    }

    // ── Dashboard ───────────────────────────────────────────

    #[test]
    fn dashboard_serialization_roundtrip() {
        let dashboard = Dashboard {
            pane: make_starter_pane(),
            popout: vec![],
        };
        let json = serde_json::to_string(&dashboard).unwrap();
        let loaded: Dashboard = serde_json::from_str(&json).unwrap();
        assert!(loaded.popout.is_empty());
    }

    #[test]
    fn dashboard_with_popout_roundtrip() {
        let dashboard = Dashboard {
            pane: make_starter_pane(),
            popout: vec![(
                make_candlestick_pane(),
                WindowSpec {
                    x: Some(100),
                    y: Some(200),
                    width: 800,
                    height: 600,
                },
            )],
        };
        let json = serde_json::to_string(&dashboard).unwrap();
        let loaded: Dashboard = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.popout.len(), 1);
        assert_eq!(loaded.popout[0].1.width, 800);
    }

    // ── LayoutManager ───────────────────────────────────────

    #[test]
    fn layout_manager_new() {
        let dashboard = Dashboard {
            pane: make_starter_pane(),
            popout: vec![],
        };
        let manager = LayoutManager::new("Default".to_string(), dashboard);
        assert_eq!(manager.layouts.len(), 1);
        assert_eq!(manager.active_layout, Some("Default".to_string()));
        assert_eq!(manager.layouts[0].name, "Default");
    }

    #[test]
    fn layout_manager_default_is_empty() {
        let manager = LayoutManager::default();
        assert!(manager.layouts.is_empty());
        assert!(manager.active_layout.is_none());
    }

    #[test]
    fn layout_manager_serialization_roundtrip() {
        let dashboard = Dashboard {
            pane: make_starter_pane(),
            popout: vec![],
        };
        let manager = LayoutManager::new("Test Layout".to_string(), dashboard);
        let json = serde_json::to_string(&manager).unwrap();
        let loaded: LayoutManager = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.layouts.len(), 1);
        assert_eq!(loaded.active_layout, Some("Test Layout".to_string()));
    }

    // ── Layouts ─────────────────────────────────────────────

    #[test]
    fn layouts_default() {
        let layouts = Layouts::default();
        assert!(layouts.items.is_empty());
        assert_eq!(layouts.current_index, 0);
    }
}
