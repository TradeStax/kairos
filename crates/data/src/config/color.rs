//! Data-layer color type (no GUI dependency).
//!
//! Use this type in persisted state and domain; convert to/from `iced::Color` at the GUI boundary.

use serde::{Deserialize, Serialize};

/// RGBA color with components in [0.0, 1.0]. Used in state and config instead of `iced_core::Color`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Rgba {
    fn default() -> Self {
        Self {
            r: 0.3,
            g: 0.6,
            b: 1.0,
            a: 1.0,
        }
    }
}

impl Rgba {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
            a: a.clamp(0.0, 1.0),
        }
    }

    pub fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: 1.0,
        }
    }

    /// Const-friendly RGB8 constructor for use in `const` / `static` contexts.
    pub const fn from_rgb8_const(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    pub fn into_rgba8(self) -> [u8; 4] {
        [
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8,
            (self.a * 255.0).round() as u8,
        ]
    }
}

/// Parse hex color "#RRGGBB" or "#RRGGBBAA". Returns None on invalid input.
pub fn hex_to_rgba(hex: &str) -> Option<Rgba> {
    if hex.len() == 7 || hex.len() == 9 {
        let hash = &hex[0..1];
        let r = u8::from_str_radix(&hex[1..3], 16);
        let g = u8::from_str_radix(&hex[3..5], 16);
        let b = u8::from_str_radix(&hex[5..7], 16);
        let a = (hex.len() == 9)
            .then(|| u8::from_str_radix(&hex[7..9], 16).ok())
            .flatten();

        return match (hash, r, g, b, a) {
            ("#", Ok(r), Ok(g), Ok(b), None) => Some(Rgba {
                r: f32::from(r) / 255.0,
                g: f32::from(g) / 255.0,
                b: f32::from(b) / 255.0,
                a: 1.0,
            }),
            ("#", Ok(r), Ok(g), Ok(b), Some(a)) => Some(Rgba {
                r: f32::from(r) / 255.0,
                g: f32::from(g) / 255.0,
                b: f32::from(b) / 255.0,
                a: f32::from(a) / 255.0,
            }),
            _ => None,
        };
    }
    None
}

/// Format Rgba as "#RRGGBB" or "#RRGGBBAA" if alpha < 1.
pub fn rgba_to_hex(color: Rgba) -> String {
    use std::fmt::Write;
    let [r, g, b, a] = color.into_rgba8();
    let mut hex = String::with_capacity(9);
    let _ = write!(&mut hex, "#{r:02X}{g:02X}{b:02X}");
    if a < u8::MAX {
        let _ = write!(&mut hex, "{a:02X}");
    }
    hex
}
