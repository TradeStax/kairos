//! Theme configuration (data-only; no iced dependency).
//!
//! Serialization format is compatible with the previous
//! iced_core::Theme-based format: built-in themes as string IDs,
//! custom theme as `{ "name": "custom", "palette": SerPalette }`.
//! The GUI crate converts `Theme` to/from `iced_core::Theme` at the
//! boundary.

use data::{Rgba, hex_to_rgba, rgba_to_hex};
use palette::{FromColor, Hsva, RgbHue};
use serde::{Deserialize, Serialize};

/// Theme identifier and optional custom palette. No iced dependency.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub id: String,
    pub custom_palette: Option<SerPalette>,
}

/// Palette for custom theme (mirrors iced_core::theme::Palette shape
/// for wire compatibility).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerPalette {
    pub background: Rgba,
    pub text: Rgba,
    pub primary: Rgba,
    pub success: Rgba,
    pub danger: Rgba,
    pub warning: Rgba,
}

#[derive(Serialize, Deserialize)]
struct SerTheme {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    palette: Option<SerPalette>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            id: "kairos".to_string(),
            custom_palette: None,
        }
    }
}

pub fn default_theme_palette() -> SerPalette {
    SerPalette {
        background: Rgba::from_rgb8(24, 22, 22),
        text: Rgba::from_rgb8(197, 201, 197),
        primary: Rgba::from_rgb8(200, 200, 200),
        success: Rgba::from_rgb8(81, 205, 160),
        danger: Rgba::from_rgb8(192, 80, 77),
        warning: Rgba::from_rgb8(238, 216, 139),
    }
}

impl Serialize for Theme {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.id == "custom" {
            if let Some(ref palette) = self.custom_palette {
                let ser = SerTheme {
                    name: "custom".to_string(),
                    palette: Some(palette.clone()),
                };
                ser.serialize(serializer)
            } else {
                self.id.serialize(serializer)
            }
        } else {
            self.id.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value =
            serde_json::Value::deserialize(deserializer).map_err(serde::de::Error::custom)?;

        if let Some(s) = value.as_str() {
            let valid = [
                "ferra",
                "dark",
                "light",
                "dracula",
                "nord",
                "solarized_light",
                "solarized_dark",
                "gruvbox_light",
                "gruvbox_dark",
                "catppuccino_latte",
                "catppuccino_frappe",
                "catppuccino_macchiato",
                "catppuccino_mocha",
                "tokyo_night",
                "tokyo_night_storm",
                "tokyo_night_light",
                "kanagawa_wave",
                "kanagawa_dragon",
                "kanagawa_lotus",
                "moonfly",
                "nightfly",
                "oxocarbon",
                "kairos",
            ];
            if valid.contains(&s) {
                return Ok(Theme {
                    id: s.to_string(),
                    custom_palette: None,
                });
            }
            return Err(serde::de::Error::custom(format!("Invalid theme: {}", s)));
        }

        let serialized = SerTheme::deserialize(value).map_err(serde::de::Error::custom)?;
        let theme = match serialized.name.as_str() {
            "kairos" => Theme::default(),
            "custom" => {
                let palette = serialized
                    .palette
                    .ok_or_else(|| serde::de::Error::custom("Custom theme missing palette data"))?;
                Theme {
                    id: "custom".to_string(),
                    custom_palette: Some(palette),
                }
            }
            _ => return Err(serde::de::Error::custom("Invalid theme")),
        };
        Ok(theme)
    }
}

// --- Helpers that work with Rgba (used by theme editor via GUI
// boundary) ---

pub fn rgba_to_hex_string(color: Rgba) -> String {
    rgba_to_hex(color)
}

pub fn hex_to_rgba_safe(hex: &str) -> Option<Rgba> {
    hex_to_rgba(hex)
}

/// Convert Rgba to palette Hsva for theme editor.
pub fn rgba_to_hsva(color: Rgba) -> Hsva {
    let srgba = palette::Srgba::new(color.r, color.g, color.b, color.a);
    Hsva::from_color(srgba)
}

/// Convert palette Hsva to Rgba.
pub fn hsva_to_rgba(hsva: Hsva) -> Rgba {
    let srgba = palette::Srgba::from_color(hsva);
    Rgba::new(srgba.red, srgba.green, srgba.blue, srgba.alpha)
}

pub fn is_dark_rgba(color: Rgba) -> bool {
    (color.r * 299.0 + color.g * 587.0 + color.b * 114.0) / 1000.0 < 0.5
}

/// Hue in degrees [0, 360), s and v in [0, 1].
pub fn from_hsv_degrees_rgba(h_deg: f32, s: f32, v: f32) -> Rgba {
    let hue = RgbHue::from_degrees(h_deg);
    hsva_to_rgba(Hsva::new(hue, s, v, 1.0))
}
