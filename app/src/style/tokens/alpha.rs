//! Alpha (opacity) scale.

pub const FAINT: f32 = 0.2; // Disabled, dark-theme hints
pub const SUBTLE: f32 = 0.3; // Faint backgrounds
pub const LIGHT: f32 = 0.4; // Cards, weak shadows
pub const MEDIUM: f32 = 0.5; // Pane grids
pub const STRONG: f32 = 0.6; // Mid-tone backgrounds
pub const HOVER_DIM: f32 = 0.7; // Subtle dimming on hover for bubble/card backgrounds
pub const HEAVY: f32 = 0.8; // Heavy shadows, dashed lines
pub const BACKDROP: f32 = 0.8; // Modal backdrop overlay — same value as HEAVY by design;
// kept separate for semantic clarity (different use context)
pub const OPAQUE: f32 = 0.99; // Modal backgrounds (near-opaque, avoids pure-white flash)
