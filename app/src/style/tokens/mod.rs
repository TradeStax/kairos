//! Design tokens -- single source of truth for all visual constants.
//! All view code should reference these instead of magic numbers.
//!
//! Organized into sub-modules by concern:
//! - `spacing` — 4px base grid scale
//! - `text` — typography sizes and font weights
//! - `radius` / `border` — border radii and widths
//! - `shadow` — shadow blur levels
//! - `alpha` — opacity scale
//! - `layout` — app shell dimensions
//! - `chart` — chart-specific constants
//! - `component` — component-specific sizes (icons, forms, buttons, etc.)
//! - `calendar` — calendar widget colors
//! - `backtest` — backtest chart colors

pub mod spacing;

// ── Path aliases ─────────────────────────────────────────────────────────────
// Some token modules are exposed under different names than their file names.
// This is intentional: the token paths (e.g. tokens::text::BODY) are stable
// public API; the file names use more descriptive English.
//
// Mapping:
//   tokens::text::*     ← typography.rs   (file: descriptive; token path: short)
//   tokens::radius::*   ← border.rs       (both radius and border live in one file)
//   tokens::border::*   ← border.rs::width (re-exported as `border` for tokens::border::THIN)
//
// Do not rename these files — the token paths are referenced throughout the codebase.

/// Typography: text sizes and font weights.
/// Exposed as `text` to maintain `tokens::text::BODY` paths.
#[path = "typography.rs"]
pub mod text;

// border.rs contains both `radius` and `width` sub-modules.
// Re-exported at top level to maintain `tokens::radius::*` and `tokens::border::*`.
#[path = "border.rs"]
mod border_mod;
pub use border_mod::radius;
pub use border_mod::width as border;

pub mod alpha;
pub mod backtest;
pub mod calendar;
pub mod chart;
pub mod component;
pub mod layout;
pub mod shadow;
