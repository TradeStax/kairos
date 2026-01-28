//! Autoscaling Logic
//!
//! Provides autoscaling calculations for chart view positioning.

use data::Autoscale;
use iced::Vector;

/// Calculate autoscaled translation for centering on latest data
///
/// Returns a vector that will center the view on the latest price/time.
pub fn calculate_autoscale_translation(
    autoscale: Option<Autoscale>,
    latest_x: f32,
    latest_y: f32,
    current_translation: Vector,
) -> Vector {
    match autoscale {
        Some(Autoscale::CenterLatest) => {
            // Center on latest point
            Vector::new(-latest_x, -latest_y)
        }
        Some(Autoscale::FitAll) => {
            // Keep X translation, but center Y
            Vector::new(current_translation.x, -latest_y)
        }
        Some(Autoscale::Disabled) | None => {
            // No change
            current_translation
        }
    }
}

/// Determine next autoscale mode when toggling
pub fn toggle_autoscale(current: Option<Autoscale>, supports_fit: bool) -> Option<Autoscale> {
    match current {
        None => Some(Autoscale::CenterLatest),
        Some(Autoscale::CenterLatest) => {
            if supports_fit {
                Some(Autoscale::FitAll)
            } else {
                Some(Autoscale::Disabled)
            }
        }
        Some(Autoscale::FitAll) => Some(Autoscale::Disabled),
        Some(Autoscale::Disabled) => None,
    }
}

/// Get autoscale button text and tooltip
pub fn autoscale_display(autoscale: Option<Autoscale>) -> (&'static str, &'static str) {
    match autoscale {
        Some(Autoscale::CenterLatest) => ("C", "Center last price"),
        Some(Autoscale::FitAll) => ("A", "Auto"),
        Some(Autoscale::Disabled) => ("D", "Disabled"),
        None => ("C", "Toggle autoscaling"),
    }
}
