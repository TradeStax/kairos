//! Shared serde default value helpers.
//!
//! Centralized `#[serde(default = "...")]` functions used across
//! `state::pane` and `drawing` modules. Keeping them in one place
//! eliminates duplication and makes it easy to audit default values.

pub(crate) fn default_true() -> bool {
    true
}

pub(crate) fn default_one() -> i64 {
    1
}

pub(crate) fn default_split_value() -> i64 {
    1
}

pub(crate) fn default_max_profiles() -> i64 {
    20
}

pub(crate) fn default_va_pct() -> f32 {
    0.7
}

pub(crate) fn default_poc_width() -> f32 {
    1.5
}

pub(crate) fn default_hvn_threshold() -> f32 {
    0.85
}

pub(crate) fn default_lvn_threshold() -> f32 {
    0.15
}

pub(crate) fn default_opacity() -> f32 {
    0.7
}

pub(crate) fn default_va_fill_opacity() -> f32 {
    0.08
}

pub(crate) fn default_line_width() -> f32 {
    1.0
}

pub(crate) fn default_zone_opacity() -> f32 {
    0.15
}

pub(crate) fn default_text_font_size() -> f32 {
    13.0
}
