//! Technical studies and indicators for Kairos charts.
//!
//! `kairos-study` provides a trait-based computation system that transforms
//! market data (candles and trades) into abstract render primitives. The
//! chart rendering layer in the `app` crate converts these primitives into
//! canvas draw calls — keeping study logic free of any GUI dependency.
//!
//! # Crate layout
//!
//! | Module | Purpose |
//! |---|---|
//! | [`config`] | Parameter definitions, validation, and runtime storage |
//! | [`core`] | The [`Study`] trait and its input/metadata types |
//! | [`output`] | Render primitives: lines, bars, profiles, footprints, markers |
//! | [`studies`] | Built-in study implementations and the [`StudyRegistry`] factory |
//! | [`util`] | Shared helpers for candle extraction and statistics |
//! | [`error`] | [`StudyError`] with severity classification |

pub mod config;
pub mod core;
pub mod error;
pub mod output;
pub mod prelude;
pub mod studies;
pub mod util;

pub use studies::orderflow;

/// Default bullish color (buy/ask) — #51CDA0.
pub const BULLISH_COLOR: data::SerializableColor =
    data::SerializableColor::from_rgb8_const(81, 205, 160);

/// Default bearish color (sell/bid) — #C0504D.
pub const BEARISH_COLOR: data::SerializableColor =
    data::SerializableColor::from_rgb8_const(192, 80, 77);

/// Default neutral color — #808080.
pub const NEUTRAL_COLOR: data::SerializableColor =
    data::SerializableColor::from_rgb8_const(128, 128, 128);

// --- Core re-exports (used by app + backtest crates) ---
pub use core::{
    DiagnosticSeverity, Study, StudyCapabilities, StudyCategory, StudyDiagnostic, StudyInput,
    StudyMetadata, StudyPlacement, StudyResult, YScaleMode,
};

// --- Config re-exports ---
pub use config::LineStyleValue;
pub use config::versioning::{ConfigMigration, ParameterSchema};
pub use config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterSection, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};

// --- Error re-export ---
pub use error::StudyError;

// --- Output re-exports (only types used via top-level paths) ---
pub use output::{CandleRenderConfig, NodeDetectionMethod, StudyOutput};

// --- Registry re-exports ---
pub use studies::{StudyInfo, StudyRegistry};
