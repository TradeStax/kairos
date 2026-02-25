//! Text sizes and font weights.

pub const TINY: f32 = 10.0; // Badges, labels
pub const SMALL: f32 = 11.0; // Chart labels, panel data (AZERET_MONO)
pub const BODY: f32 = 12.0; // Default UI text
pub const LABEL: f32 = 13.0; // Form labels, section headers
pub const TITLE: f32 = 14.0; // Dialog titles, prominent text
pub const HEADING: f32 = 16.0; // Modal headings

pub mod weight {
    pub use iced::font::Weight::{Bold as BOLD, Normal as NORMAL};
}
