//! Volume Weighted Average Price (VWAP).
//!
//! VWAP computes the cumulative ratio of volume-weighted typical price to
//! total volume across the session. It answers the question: "What is the
//! average price at which volume has traded today?"
//!
//! # Formula
//!
//! ```text
//! VWAP = sum(TP * V) / sum(V)
//! ```
//!
//! where `TP = (High + Low + Close) / 3` (the typical price) and `V` is
//! the candle volume. The calculation is cumulative from the first candle
//! in the data set.
//!
//! # Trading use
//!
//! - **Execution benchmark**: institutional desks measure fill quality
//!   against VWAP. Buying below VWAP or selling above it is considered
//!   favorable execution.
//! - **Intraday bias**: price holding above VWAP signals bullish
//!   conviction; sustained trading below signals bearish pressure.
//! - **Mean reversion**: standard-deviation bands around VWAP act as
//!   overbought/oversold levels. Extreme deviations from VWAP tend to
//!   revert, especially in range-bound sessions.
//!
//! # Output
//!
//! Produces [`StudyOutput::Lines`] with the VWAP line and, when the
//! `show_bands` parameter is enabled, upper and lower standard-deviation
//! band lines.

mod params;
#[cfg(test)]
mod tests;

use crate::config::StudyConfig;
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::util::candle_key;

use params::{BAND_COLOR, DEFAULT_COLOR, make_params};

/// Volume Weighted Average Price study.
///
/// Renders the VWAP line on the price chart with optional upper/lower
/// standard deviation bands. The bands use a rolling volume-weighted
/// variance to compute standard deviation at each candle.
///
/// Configurable parameters: line color, line width, band visibility,
/// and band multiplier (number of standard deviations).
pub struct VwapStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<crate::config::ParameterDef>,
}

impl VwapStudy {
    /// Create a new VWAP study with default parameters.
    ///
    /// Defaults: cyan line, width = 1.5, bands disabled, band multiplier = 1.0.
    pub fn new() -> Self {
        let params = make_params();
        let config = StudyConfig::from_params("vwap", &params);

        Self {
            metadata: StudyMetadata {
                name: "Volume Weighted Average Price".to_string(),
                category: StudyCategory::Trend,
                placement: StudyPlacement::Overlay,
                description: "Volume weighted average price with optional bands".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for VwapStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for VwapStudy {
    crate::impl_study_base!("vwap");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let color = self.config.get_color("color", DEFAULT_COLOR);
        let width = self.config.get_float("width", 1.5) as f32;
        let show_bands = self.config.get_bool("show_bands", false);
        let band_mult = self.config.get_float("band_multiplier", 1.0);

        let candles = input.candles;
        if candles.is_empty() {
            log::debug!("{}: no candle data", self.id());
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        let mut cum_tp_vol: f64 = 0.0;
        let mut cum_vol: f64 = 0.0;
        let mut cum_tp2_vol: f64 = 0.0;

        let mut vwap_points = Vec::with_capacity(candles.len());
        let mut upper_points = Vec::with_capacity(candles.len());
        let mut lower_points = Vec::with_capacity(candles.len());

        for (i, candle) in candles.iter().enumerate() {
            let typical_price =
                (candle.high.to_f32() + candle.low.to_f32() + candle.close.to_f32()) as f64 / 3.0;
            let vol = candle.volume() as f64;

            cum_tp_vol += typical_price * vol;
            cum_vol += vol;
            cum_tp2_vol += typical_price * typical_price * vol;

            let key = candle_key(candle, i, candles.len(), &input.basis);

            if cum_vol > 0.0 {
                let vwap = cum_tp_vol / cum_vol;
                vwap_points.push((key, vwap as f32));

                if show_bands {
                    let variance = (cum_tp2_vol / cum_vol) - (vwap * vwap);
                    let std_dev = if variance > 0.0 { variance.sqrt() } else { 0.0 };
                    upper_points.push((key, (vwap + std_dev * band_mult) as f32));
                    lower_points.push((key, (vwap - std_dev * band_mult) as f32));
                }
            } else {
                // Zero cumulative volume: VWAP is undefined, so we fall back
                // to the typical price as a reasonable default. This avoids a
                // division-by-zero and keeps the line continuous through
                // zero-volume candles (e.g. pre-market placeholder bars).
                vwap_points.push((key, typical_price as f32));
                if show_bands {
                    upper_points.push((key, typical_price as f32));
                    lower_points.push((key, typical_price as f32));
                }
            }
        }

        let style = crate::config::LineStyleValue::Solid;

        let mut lines = vec![LineSeries {
            label: "VWAP".to_string(),
            color,
            width,
            style,
            points: vwap_points,
        }];

        if show_bands {
            lines.push(LineSeries {
                label: "VWAP Upper".to_string(),
                color: BAND_COLOR,
                width: width * 0.7,
                style: crate::config::LineStyleValue::Dashed,
                points: upper_points,
            });
            lines.push(LineSeries {
                label: "VWAP Lower".to_string(),
                color: BAND_COLOR,
                width: width * 0.7,
                style: crate::config::LineStyleValue::Dashed,
                points: lower_points,
            });
        }

        self.output = StudyOutput::Lines(lines);
        Ok(StudyResult::ok())
    }
}
