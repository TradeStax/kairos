//! Core study trait, input data, metadata, capabilities, and result types.
//!
//! - [`Study`] — The trait all studies implement (compute, output, config).
//! - [`StudyInput`] — Candle/trade data and chart context passed to `compute()`.
//! - [`StudyMetadata`] — Consolidated name, category, placement, capabilities.
//! - [`StudyCapabilities`] — Feature flags for optional study behaviors.
//! - [`StudyResult`] — Compute result with diagnostics and change tracking.
//! - [`StudyCategory`] — Grouping for menus: Trend, Momentum, Volume, etc.
//! - [`StudyPlacement`] — Where the study renders: Overlay, Panel, Background, etc.

pub mod capabilities;
pub mod composition;
pub mod draw_context;
pub mod input;
pub mod interactive;
pub mod metadata;
pub mod result;
pub mod study;

pub use input::StudyInput;
pub use metadata::{StudyCapabilities, StudyCategory, StudyMetadata, StudyPlacement};
pub use result::{DiagnosticSeverity, StudyDiagnostic, StudyResult};
pub use study::{Study, YScaleMode};
