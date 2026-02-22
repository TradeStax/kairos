//! Animation tokens — single source of truth for all motion constants.

pub use iced_anim::spring::Motion;

use std::time::Duration;

/// Spring presets for interactive, momentum-based animations.
pub mod spring {
    use super::*;

    /// Fast response, critically damped (no bounce).
    /// Use for: hover color transitions, status dot color, toggle states.
    pub const SUBTLE: Motion = Motion::SMOOTH;

    /// Smooth with very slight overshoot.
    /// Use for: modal backdrop fade, overlay appearance, panel transitions.
    pub const SMOOTH: Motion = Motion::SNAPPY;

    /// Moderate speed, slight bounce for expand/collapse.
    /// Use for: collapsible arrow rotation, accordion sections.
    pub const EXPAND: Motion = Motion::BOUNCY;
}

/// Duration constants for time-based animations (toast slide, etc.).
pub mod duration {
    use super::*;

    /// Toast slide-in duration.
    pub const TOAST_ENTER: Duration = Duration::from_millis(200);
    /// Toast fade-out duration.
    pub const TOAST_EXIT: Duration = Duration::from_millis(300);
}

/// Pan deceleration physics constants (not spring-based, direct velocity).
pub mod deceleration {
    /// Friction multiplier per frame (0.92 = 8% velocity loss per tick).
    pub const FRICTION: f32 = 0.92;
    /// Velocity threshold below which deceleration stops (px/frame).
    pub const STOP_THRESHOLD: f32 = 0.5;
}
