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
use super::metadata::StudyMetadata;
use super::result::StudyResult;
use crate::config::{ParameterDef, ParameterTab, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{CandleRenderConfig, StudyOutput};

/// Y-axis scale mode for panel studies.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum YScaleMode {
    /// Standard linear scale.
    #[default]
    Linear,
    /// Logarithmic (base 10) scale.
    Log10,
    /// Percentage change from first visible value.
    Percentage,
    /// Fixed range (ignores autoscale).
    Fixed { min: f32, max: f32 },
}

/// Core trait for all technical studies and indicators.
///
/// Implementors provide a `compute()` method that transforms [`StudyInput`]
/// into a [`StudyOutput`], plus configuration, metadata, and lifecycle methods.
pub trait Study: Send + Sync + 'static {
    /// Unique identifier (e.g. "sma", "rsi", "volume_profile")
    fn id(&self) -> &str;

    /// Consolidated metadata: name, category, placement, description,
    /// capabilities, and config version.
    fn metadata(&self) -> &StudyMetadata;

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

    /// Update multiple parameters atomically.
    ///
    /// Validates all parameters first, then applies all. Returns an error
    /// if any parameter fails validation (no parameters are applied).
    fn set_parameters(&mut self, params: &[(&str, ParameterValue)]) -> Result<(), StudyError> {
        let defs: Vec<_> = self.parameters().to_vec();
        crate::core::capabilities::set_parameters_default(&defs, self.config_mut(), params)
    }

    /// Recompute all study values from scratch using the provided input.
    ///
    /// Called whenever the underlying data changes (new candles loaded,
    /// visible range scrolled, parameters updated). For incremental updates
    /// on streaming data prefer `append_trades`.
    ///
    /// Returns a [`StudyResult`] indicating whether the output changed
    /// and carrying optional diagnostic messages.
    ///
    /// # Errors
    /// Returns [`StudyError`] if parameters are misconfigured or computation fails.
    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError>;

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
    ) -> Result<StudyResult, StudyError> {
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
    /// The UI layer uses the returned `&dyn Any` to downcast to a concrete
    /// type (e.g. `LevelAnalyzerData`) to populate modals and overlays.
    /// Default: `None` (no interactive data available).
    fn interactive_data(&self) -> Option<&dyn Any> {
        None
    }

    /// Accept externally-provided data (e.g. user-defined manual levels).
    ///
    /// Type-erased via `Box<dyn Any + Send>`; the study downcasts internally
    /// to its expected payload type. Default: returns an error.
    fn accept_external_data(&mut self, _data: Box<dyn Any + Send>) -> Result<(), StudyError> {
        Err(StudyError::InvalidParameter {
            key: "external_data".into(),
            reason: "not supported".into(),
        })
    }

    /// Y-axis scale mode for panel studies.
    ///
    /// Default: [`YScaleMode::Linear`]. Override for studies that need
    /// logarithmic, percentage, or fixed-range scaling.
    fn y_scale(&self) -> YScaleMode {
        YScaleMode::Linear
    }

    /// Clone this study into a new heap-allocated instance.
    ///
    /// A manual clone is required because `dyn Study` trait objects are not
    /// `Clone` (object-safe trait cloning requires indirection via a method).
    /// Implementations should deep-copy config, params, and output.
    fn clone_study(&self) -> Box<dyn Study>;
}
