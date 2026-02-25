//! Border radii and widths.

/// Border radii.
pub mod radius {
    pub const NONE: f32 = 0.0;
    pub const SM: f32 = 2.0; // Inputs, scrollbars
    pub const MD: f32 = 4.0; // Buttons, containers, modals (default)
    pub const LG: f32 = 6.0; // Emphasized panels
    pub const ROUND: f32 = 16.0; // Circles, pills
}

/// Border widths.
pub mod width {
    pub const NONE: f32 = 0.0;
    pub const THIN: f32 = 1.0; // Standard borders
    pub const MEDIUM: f32 = 1.5; // Emphasized (confirm modals)
    pub const THICK: f32 = 2.0; // Active state, scrollbars
}
