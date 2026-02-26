//! Link group for synchronized panes.

use serde::{Deserialize, Serialize};

/// Link group for synchronized panes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LinkGroup(pub u8);

impl LinkGroup {
    pub const ALL: [LinkGroup; 9] = [
        LinkGroup(1),
        LinkGroup(2),
        LinkGroup(3),
        LinkGroup(4),
        LinkGroup(5),
        LinkGroup(6),
        LinkGroup(7),
        LinkGroup(8),
        LinkGroup(9),
    ];
}

impl std::fmt::Display for LinkGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
