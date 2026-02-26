//! UI-layer configuration types
//!
//! Types for theme, timezone, sidebar, secrets, and scaling that
//! belong to the application layer rather than the data crate.

pub mod scale_factor;
pub mod secrets;
pub mod sidebar;
pub mod theme;
pub mod timezone;

pub use scale_factor::ScaleFactor;
pub use sidebar::Sidebar;
pub use theme::Theme;
pub use timezone::UserTimezone;
