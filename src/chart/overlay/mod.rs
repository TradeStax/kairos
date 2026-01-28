//! Chart Overlay System
//!
//! Provides overlay rendering for charts:
//! - `Crosshair` - Crosshair lines following cursor
//! - `Ruler` - Measurement tool for price/time differences
//! - `LastPriceLine` - Horizontal line at last traded price
//! - `Grid` - Background grid lines

mod crosshair;
mod grid;
mod last_price;
mod ruler;

pub use crosshair::{draw_crosshair, CrosshairResult};
pub use grid::draw_grid;
pub use last_price::draw_last_price_line;
pub use ruler::draw_ruler;

use iced::theme::palette::Extended;
use iced::widget::canvas::Frame;

/// Overlay trait for chart overlays
pub trait Overlay {
    /// Draw the overlay onto the frame
    fn draw(&self, frame: &mut Frame, palette: &Extended);

    /// Whether this overlay is currently visible
    fn is_visible(&self) -> bool {
        true
    }

    /// Priority for draw order (higher = drawn later/on top)
    fn priority(&self) -> u8 {
        0
    }
}
