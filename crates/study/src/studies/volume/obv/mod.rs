//! On Balance Volume (OBV).
//!
//! Cumulative indicator: adds the candle's volume when the close is higher
//! than the previous close, subtracts it when lower.
//! Formula: `OBV(t) = OBV(t-1) + sign(Close(t) - Close(t-1)) * Volume(t)`
//!
//! Output: `StudyOutput::Lines` — a single cumulative line.

mod params;
#[cfg(test)]
mod tests;

use crate::config::{LineStyleValue, StudyConfig};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::util::candle_key;
use params::DEFAULT_COLOR;

/// On-Balance Volume line study.
///
/// Each candle contributes its total volume with a sign determined by
/// the close-over-close direction: `OBV(t) = OBV(t-1) + sign * Vol(t)`.
/// Rising OBV confirms buying conviction behind an uptrend; falling OBV
/// confirms selling conviction behind a downtrend. Divergences between
/// OBV and price often precede trend reversals.
///
/// Renders as a single cumulative line in a separate panel.
pub struct ObvStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<crate::config::ParameterDef>,
}

impl ObvStudy {
    /// Create a new OBV study with a white line at 1.5px width.
    pub fn new() -> Self {
        let params = params::make_params();
        let config = StudyConfig::from_params("obv", &params);

        Self {
            metadata: StudyMetadata {
                name: "On Balance Volume".to_string(),
                category: StudyCategory::Volume,
                placement: StudyPlacement::Panel,
                description: "Cumulative volume based on price direction".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for ObvStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for ObvStudy {
    crate::impl_study_base!("obv");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let color = self.config.get_color("color", DEFAULT_COLOR);
        let width = self.config.get_float("width", 1.5) as f32;

        let candles = input.candles;
        if candles.is_empty() {
            log::debug!("{}: no candle data", self.id());
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        let mut obv: f64 = 0.0;
        let mut points = Vec::with_capacity(candles.len());

        // First candle: OBV starts at 0
        let key = candle_key(&candles[0], 0, candles.len(), &input.basis);
        points.push((key, obv as f32));

        for i in 1..candles.len() {
            let close = candles[i].close.to_f32();
            let prev_close = candles[i - 1].close.to_f32();
            let vol = candles[i].volume() as f64;

            if close > prev_close {
                obv += vol;
            } else if close < prev_close {
                obv -= vol;
            }
            // If equal, OBV unchanged

            let key = candle_key(&candles[i], i, candles.len(), &input.basis);
            points.push((key, obv as f32));
        }

        self.output = StudyOutput::Lines(vec![LineSeries {
            label: "OBV".to_string(),
            color,
            width,
            style: LineStyleValue::Solid,
            points,
        }]);
        Ok(StudyResult::ok())
    }
}
