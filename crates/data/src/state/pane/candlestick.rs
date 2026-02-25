//! Candlestick chart configuration.

use serde::{Deserialize, Serialize};

/// Which candlestick color field is currently being edited in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandleColorField {
    BullBody,
    BearBody,
    BullWick,
    BearWick,
    BullBorder,
    BearBorder,
}

/// Candlestick visual style configuration.
///
/// Each field is `Option<Rgba>` — `None` means "use theme palette default".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CandleStyle {
    pub bull_body_color: Option<crate::config::color::Rgba>,
    pub bear_body_color: Option<crate::config::color::Rgba>,
    pub bull_wick_color: Option<crate::config::color::Rgba>,
    pub bear_wick_color: Option<crate::config::color::Rgba>,
    pub bull_border_color: Option<crate::config::color::Rgba>,
    pub bear_border_color: Option<crate::config::color::Rgba>,
    /// When true, candle body opacity scales with volume (high volume = more opaque).
    #[serde(default)]
    pub volume_opacity: bool,
}

impl CandleStyle {
    /// Get the color for a given field.
    pub fn get_color(&self, field: CandleColorField) -> Option<crate::config::color::Rgba> {
        match field {
            CandleColorField::BullBody => self.bull_body_color,
            CandleColorField::BearBody => self.bear_body_color,
            CandleColorField::BullWick => self.bull_wick_color,
            CandleColorField::BearWick => self.bear_wick_color,
            CandleColorField::BullBorder => self.bull_border_color,
            CandleColorField::BearBorder => self.bear_border_color,
        }
    }

    /// Set the color for a given field.
    pub fn set_color(
        &mut self,
        field: CandleColorField,
        color: Option<crate::config::color::Rgba>,
    ) {
        match field {
            CandleColorField::BullBody => self.bull_body_color = color,
            CandleColorField::BearBody => self.bear_body_color = color,
            CandleColorField::BullWick => self.bull_wick_color = color,
            CandleColorField::BearWick => self.bear_wick_color = color,
            CandleColorField::BullBorder => self.bull_border_color = color,
            CandleColorField::BearBorder => self.bear_border_color = color,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KlineConfig {
    /// PERSISTED — whether the volume sub-chart is visible.
    pub show_volume: bool,
    /// PERSISTED — color scheme identifier (e.g. "default").
    pub color_scheme: String,
    /// PERSISTED — candlestick visual style (body/wick/border colors).
    #[serde(default)]
    pub candle_style: CandleStyle,
    /// RUNTIME ONLY — which color field is currently being edited in the UI.
    /// Skipped during serialization; always `None` on load.
    #[serde(skip)]
    pub editing_color: Option<CandleColorField>,
    /// RUNTIME ONLY — whether to show debug performance overlay (FPS, frame time, etc.).
    #[serde(skip)]
    pub show_debug_info: bool,
}
