//! Volume Delta.
//!
//! Per-candle delta: buy volume minus sell volume. Positive delta indicates
//! net buying pressure; negative delta indicates net selling pressure.
//!
//! Requires trade-level data (`StudyInput::trades`). Returns `StudyOutput::Empty`
//! if no trade data is available.
//!
//! Output: `StudyOutput::Bars` — one bar per candle, colored by sign.

mod params;
#[cfg(test)]
mod tests;

use crate::config::StudyConfig;
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{BarPoint, BarSeries, StudyOutput};
use crate::util::candle_key;
use data::SerializableColor;
use params::{DEFAULT_NEG_COLOR, DEFAULT_OPACITY, DEFAULT_POS_COLOR};

/// Per-candle volume delta (buy volume minus sell volume).
///
/// Positive delta means more contracts traded at the ask (aggressive
/// buyers); negative delta means more traded at the bid (aggressive
/// sellers). Renders as colored bars in a separate panel below the
/// price chart.
pub struct DeltaStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<crate::config::ParameterDef>,
}

impl DeltaStudy {
    /// Create a new Delta study with default bullish/bearish colors
    /// and 80% opacity.
    pub fn new() -> Self {
        let params = params::make_params();
        let config = StudyConfig::from_params("delta", &params);

        Self {
            metadata: StudyMetadata {
                name: "Volume Delta".to_string(),
                category: StudyCategory::Volume,
                placement: StudyPlacement::Panel,
                description: "Buy minus sell volume per candle".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for DeltaStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for DeltaStudy {
    crate::impl_study_base!("delta");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let pos_color = self.config.get_color("positive_color", DEFAULT_POS_COLOR);
        let neg_color = self.config.get_color("negative_color", DEFAULT_NEG_COLOR);
        let opacity = self.config.get_float("opacity", DEFAULT_OPACITY) as f32;

        if input.candles.is_empty() {
            log::debug!("{}: no candle data", self.id());
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        let total = input.candles.len();
        let points: Vec<BarPoint> = input
            .candles
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let delta = c.volume_delta() as f32;
                let base_color = if delta >= 0.0 { pos_color } else { neg_color };
                BarPoint {
                    x: candle_key(c, i, total, &input.basis),
                    value: delta,
                    color: SerializableColor::new(
                        base_color.r,
                        base_color.g,
                        base_color.b,
                        opacity,
                    ),
                    overlay: None,
                }
            })
            .collect();

        self.output = StudyOutput::Bars(vec![BarSeries {
            label: "Delta".to_string(),
            points,
        }]);
        Ok(StudyResult::ok())
    }
}
