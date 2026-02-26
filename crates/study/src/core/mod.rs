//! Core study trait, input data, and classification metadata.
//!
//! - [`Study`] — The trait all studies implement (compute, output, config).
//! - [`StudyInput`] — Candle/trade data and chart context passed to `compute()`.
//! - [`StudyCategory`] — Grouping for menus: Trend, Momentum, Volume, etc.
//! - [`StudyPlacement`] — Where the study renders: Overlay, Panel, Background, etc.

pub mod input;
pub mod metadata;
pub mod study;

pub use input::StudyInput;
pub use metadata::{StudyCategory, StudyPlacement};
pub use study::Study;
