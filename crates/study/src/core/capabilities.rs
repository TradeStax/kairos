//! Capability traits for optional study features.
//!
//! Studies that provide advanced features (incremental updates, interactive
//! data, custom tab labels, etc.) implement the corresponding capability
//! trait in addition to the core [`Study`](super::Study) trait.
//!
//! The core trait retains default-impl methods for backward compatibility —
//! these capability traits provide a type-safe opt-in interface for future
//! composition and discovery.

use data::Trade;

use super::input::StudyInput;
use super::interactive::{CrosshairValue, InteractivePayload, InteractiveRegion, StudyModalSpec};
use super::result::StudyResult;
use super::study::Study;
use crate::config::{ParameterTab, ParameterValue};
use crate::error::StudyError;
use crate::output::CandleRenderConfig;

/// Studies that support incremental trade-by-trade updates.
///
/// Implementing this trait signals that the study maintains running state
/// and can efficiently process new trades without a full recompute.
pub trait IncrementalStudy: Study {
    /// Process only newly arrived trades.
    fn append_trades(
        &mut self,
        new_trades: &[Trade],
        input: &StudyInput,
    ) -> Result<StudyResult, StudyError>;
}

/// Studies that provide interactive data for UI overlays and modals.
pub trait InteractiveStudy: Study {
    /// Type identifier for the interactive payload.
    fn interactive_type_id(&self) -> &str;

    /// Structured data for UI modals and overlays.
    fn interactive_data(&self) -> Option<InteractivePayload>;

    /// Clickable/hoverable regions on the chart.
    fn interactive_regions(&self) -> Vec<InteractiveRegion> {
        vec![]
    }

    /// Handle drag-adjust interactions on chart elements.
    fn handle_drag_adjust(&mut self, _key: &str, _new_value: f64) -> bool {
        false
    }

    /// Values to display next to the crosshair at a given interval.
    fn crosshair_values(&self, _at_interval: u64) -> Vec<CrosshairValue> {
        vec![]
    }

    /// Specification for a detail modal (sections with key-value rows).
    fn detail_modal_spec(&self) -> Option<StudyModalSpec> {
        None
    }
}

/// Studies that consume data from other studies.
///
/// Extension point: no built-in studies implement this yet.
pub trait CompositeStudy: Study {
    /// Declare study dependencies by ID.
    fn dependencies(&self) -> &[super::composition::StudyDependency];

    /// Compute using resolved dependency outputs.
    fn compute_with_deps(
        &mut self,
        input: &StudyInput,
        deps: &super::composition::DependencyOutputs,
    ) -> Result<StudyResult, StudyError>;
}

/// Studies that accept externally-provided data (e.g. manual levels).
pub trait ExternalDataStudy: Study {
    /// Data type identifiers this study accepts.
    fn accepted_data_types(&self) -> &[&str];

    /// Accept external data by type identifier.
    fn accept_data(
        &mut self,
        type_id: &str,
        data: Box<dyn std::any::Any + Send>,
    ) -> Result<(), StudyError>;
}

/// Studies that replace the standard candlestick rendering.
pub trait CandleReplaceStudy: Study {
    /// Layout overrides for cell sizing, zoom bounds, etc.
    fn candle_render_config(&self) -> CandleRenderConfig;
}

/// Studies with custom tab labels in the settings modal.
///
/// Extension point: no built-in studies implement this yet.
pub trait CustomTabStudy: Study {
    /// Custom (label, tab) pairs for the settings UI.
    fn tab_labels(&self) -> &[(&'static str, ParameterTab)];
}

/// Studies that use the platform-agnostic drawing API.
///
/// Extension point: no built-in studies implement this yet.
pub trait CustomDrawStudy: Study {
    /// Draw custom visuals using the provided draw context.
    fn custom_draw(&self, ctx: &mut dyn super::draw_context::DrawContext);
}

/// Batch parameter update: validates all parameters first, then applies atomically.
pub fn set_parameters_default(
    parameters: &[crate::config::ParameterDef],
    config: &mut crate::config::StudyConfig,
    params: &[(&str, ParameterValue)],
) -> Result<(), StudyError> {
    // Validate all first
    for (key, value) in params {
        let def = parameters.iter().find(|p| p.key == *key).ok_or_else(|| {
            StudyError::InvalidParameter {
                key: key.to_string(),
                reason: "unknown parameter".to_string(),
            }
        })?;
        def.validate(value)
            .map_err(|reason| StudyError::InvalidParameter {
                key: key.to_string(),
                reason,
            })?;
    }

    // Apply all
    for (key, value) in params {
        config.set(key.to_string(), value.clone());
    }
    Ok(())
}
