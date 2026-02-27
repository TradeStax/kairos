//! The [`Study`] trait — the central abstraction of this crate.
//!
//! Every technical indicator, overlay, and order-flow visualization
//! implements [`Study`]. The chart engine drives the lifecycle:
//!
//! 1. Call [`compute()`](Study::compute) with a [`StudyInput`] whenever
//!    candle data changes or parameters are updated.
//! 2. Read back render primitives via [`output()`](Study::output).
//! 3. Optionally call [`append_trades()`](Study::append_trades) for
//!    incremental streaming updates without a full recompute.
//! 4. Call [`reset()`](Study::reset) when the chart is cleared or the
//!    instrument changes.

use std::any::Any;

use data::Trade;

use super::input::StudyInput;
use super::metadata::{StudyCategory, StudyPlacement};
use crate::config::{ParameterDef, ParameterTab, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{CandleRenderConfig, StudyOutput};

/// Core trait for all technical studies and indicators.
///
/// Implementors provide a `compute()` method that transforms [`StudyInput`]
/// into a [`StudyOutput`], plus configuration, metadata, and lifecycle methods.
pub trait Study: Send + Sync {
    /// Unique identifier (e.g. "sma", "rsi", "volume_profile")
    fn id(&self) -> &str;

    /// Display name (e.g. "Simple Moving Average")
    fn name(&self) -> &str;

    /// Category for grouping in the UI
    fn category(&self) -> StudyCategory;

    /// Where this study renders relative to the price chart.
    ///
    /// Dynamic studies (e.g. VBP) may return different placements based on
    /// their current config (e.g. `In Chart` vs `Side Panel`).
    fn placement(&self) -> StudyPlacement;

    /// Parameter definitions for the settings UI
    fn parameters(&self) -> &[ParameterDef];

    /// Current configuration snapshot
    fn config(&self) -> &StudyConfig;

    /// Mutable access to configuration for the default
    /// `set_parameter` implementation.
    fn config_mut(&mut self) -> &mut StudyConfig;

    /// Update a single parameter by key.
    ///
    /// Default implementation validates against `parameters()` definitions
    /// and sets the value. Override only if custom cross-field validation
    /// is needed.
    fn set_parameter(&mut self, key: &str, value: ParameterValue) -> Result<(), StudyError> {
        // Borrow parameters slice before mutable borrow of config
        let params = self.parameters();
        let def =
            params
                .iter()
                .find(|p| p.key == key)
                .ok_or_else(|| StudyError::InvalidParameter {
                    key: key.to_string(),
                    reason: "unknown parameter".to_string(),
                })?;

        def.validate(&value)
            .map_err(|reason| StudyError::InvalidParameter {
                key: key.to_string(),
                reason,
            })?;

        self.config_mut().set(key, value);
        Ok(())
    }

    /// Recompute all study values from scratch using the provided input.
    ///
    /// Called whenever the underlying data changes (new candles loaded,
    /// visible range scrolled, parameters updated). For incremental updates
    /// on streaming data prefer `append_trades`.
    ///
    /// # Errors
    /// Returns [`StudyError`] if parameters are misconfigured or computation fails.
    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError>;

    /// Incrementally process new trades appended since last compute.
    ///
    /// Override this for studies that maintain running state (e.g. CVD, Big Trades)
    /// to avoid the O(n) cost of a full recompute. The default implementation
    /// falls back to a full `compute` call.
    ///
    /// `new_trades` contains only trades appended since the last call.
    /// `input` contains the full up-to-date candle + trade slice.
    fn append_trades(
        &mut self,
        _new_trades: &[Trade],
        input: &StudyInput,
    ) -> Result<(), StudyError> {
        self.compute(input)
    }

    /// Return the last computed output, ready for the renderer.
    ///
    /// Returns `StudyOutput::Empty` before the first successful `compute()` or
    /// after `reset()` is called.
    fn output(&self) -> &StudyOutput;

    /// Clear all computed data and return to initial state.
    ///
    /// Called when the chart is cleared, the ticker changes, or a replay is rewound.
    /// After `reset()`, `output()` must return `StudyOutput::Empty`.
    fn reset(&mut self);

    /// Optional layout overrides for `CandleReplace` studies.
    ///
    /// Returns constants that override the chart's default cell sizing, zoom
    /// bounds, and initial candle window. At most one `CandleReplace` study
    /// can be active at a time; the chart engine enforces this constraint.
    fn candle_render_config(&self) -> Option<CandleRenderConfig> {
        None
    }

    /// Optional custom tab labels for the settings UI.
    /// Returns (label, tab) pairs. When None, default tab names are used.
    fn tab_labels(&self) -> Option<&[(&'static str, ParameterTab)]> {
        None
    }

    /// Structured data for interactive UI features (e.g. level detail modal).
    ///
    /// The UI layer downcasts the returned `&dyn Any` to a concrete type
    /// (e.g. `LevelAnalyzerData`) to populate modals and overlays.
    /// Default: `None` (no interactive data available).
    fn interactive_data(&self) -> Option<&dyn Any> {
        None
    }

    /// Whether this study has a detail modal accessible from the overlay.
    ///
    /// When `true`, the chart overlay renders an icon button next to the
    /// study label that opens the detail modal on click.
    fn has_detail_modal(&self) -> bool {
        false
    }

    /// Whether this study depends on the visible range and should
    /// recompute when the user scrolls, pans, or zooms.
    ///
    /// Default: `false`. Override to `true` for studies like VBP that
    /// compute over the visible window.
    fn needs_visible_range(&self) -> bool {
        false
    }

    /// Accept externally-provided data (e.g. user-defined manual levels).
    ///
    /// Type-erased via `Box<dyn Any + Send>`; the study downcasts internally
    /// to its expected payload type. Default: returns an error.
    fn accept_external_data(
        &mut self,
        _data: Box<dyn Any + Send>,
    ) -> Result<(), StudyError> {
        Err(StudyError::InvalidParameter {
            key: "external_data".into(),
            reason: "not supported".into(),
        })
    }

    /// Clone this study into a new heap-allocated instance.
    ///
    /// A manual clone is required because `dyn Study` trait objects are not
    /// `Clone` (object-safe trait cloning requires indirection via a method).
    /// Implementations should deep-copy config, params, and output.
    fn clone_study(&self) -> Box<dyn Study>;
}
