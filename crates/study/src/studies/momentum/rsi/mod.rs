//! Relative Strength Index (RSI).
//!
//! RSI measures the speed and magnitude of recent price changes to evaluate
//! overbought or oversold conditions. It oscillates between 0 and 100.
//!
//! Uses Wilder's smoothed moving average: after seeding from the first
//! `period` price changes, each subsequent value blends the previous average
//! with the current gain or loss using the factor `(period - 1) / period`.
//!
//! Traders typically watch for readings above 70 (overbought) or below 30
//! (oversold), divergences between RSI and price, and centerline (50)
//! crossovers for trend confirmation.
//!
//! Output: `StudyOutput::Composite` containing a line series and two
//! horizontal levels (overbought / oversold).

mod params;
#[cfg(test)]
mod tests;

use crate::config::{LineStyleValue, StudyConfig};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{LineSeries, PriceLevel, StudyOutput};
use crate::util::candle_key;
use data::SerializableColor;
use params::{LEVEL_COLOR, make_params};

/// Relative Strength Index oscillator.
///
/// Computes the ratio of average gains to average losses over the
/// configured lookback period using Wilder's smoothing, then maps the
/// result to a 0--100 scale. Renders as a single line in a separate
/// panel with configurable overbought and oversold reference levels.
pub struct RsiStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<crate::config::ParameterDef>,
}

impl RsiStudy {
    /// Create a new RSI study with the standard default parameters:
    /// 14-period lookback, overbought level at 70, oversold level at 30.
    pub fn new() -> Self {
        let params = make_params();
        let config = StudyConfig::from_params("rsi", &params);

        Self {
            metadata: StudyMetadata {
                name: "Relative Strength Index".to_string(),
                category: StudyCategory::Momentum,
                placement: StudyPlacement::Panel,
                description: "Momentum oscillator measuring overbought/oversold conditions"
                    .to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for RsiStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for RsiStudy {
    crate::impl_study_base!("rsi");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let period = self.config.get_int("period", 14) as usize;
        let ob = self.config.get_float("overbought", 70.0);
        let os = self.config.get_float("oversold", 30.0);
        let color = self.config.get_color(
            "color",
            SerializableColor {
                r: 1.0,
                g: 0.85,
                b: 0.2,
                a: 1.0,
            },
        );

        let candles = input.candles;
        // Need at least period + 1 candles to compute one RSI value
        if candles.len() < period + 1 {
            log::debug!(
                "{}: insufficient data ({} candles, need {})",
                self.id(),
                candles.len(),
                period + 1
            );
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        let closes: Vec<f64> = candles.iter().map(|c| c.close.to_f64()).collect();

        // Calculate initial average gain and loss from first `period` changes
        let mut avg_gain: f64 = 0.0;
        let mut avg_loss: f64 = 0.0;
        for i in 1..=period {
            let change = closes[i] - closes[i - 1];
            if change > 0.0 {
                avg_gain += change;
            } else {
                avg_loss += -change;
            }
        }
        avg_gain /= period as f64;
        avg_loss /= period as f64;

        let mut points = Vec::with_capacity(candles.len() - period);

        // First RSI value
        let rsi = if avg_loss == 0.0 {
            100.0
        } else {
            100.0 - 100.0 / (1.0 + avg_gain / avg_loss)
        };
        points.push((
            candle_key(&candles[period], period, candles.len(), &input.basis),
            rsi as f32,
        ));

        // Wilder's smoothing for subsequent values
        for i in (period + 1)..candles.len() {
            let change = closes[i] - closes[i - 1];
            let (gain, loss) = if change > 0.0 {
                (change, 0.0)
            } else {
                (0.0, -change)
            };

            avg_gain = (avg_gain * (period as f64 - 1.0) + gain) / period as f64;
            avg_loss = (avg_loss * (period as f64 - 1.0) + loss) / period as f64;

            let rsi = if avg_loss == 0.0 {
                100.0
            } else {
                100.0 - 100.0 / (1.0 + avg_gain / avg_loss)
            };
            points.push((
                candle_key(&candles[i], i, candles.len(), &input.basis),
                rsi as f32,
            ));
        }

        let levels = vec![
            PriceLevel::horizontal(ob, format!("OB {ob}"), LEVEL_COLOR)
                .with_style(LineStyleValue::Dashed)
                .with_opacity(0.6),
            PriceLevel::horizontal(os, format!("OS {os}"), LEVEL_COLOR)
                .with_style(LineStyleValue::Dashed)
                .with_opacity(0.6),
        ];

        self.output = StudyOutput::Composite(vec![
            StudyOutput::Lines(vec![LineSeries {
                label: format!("RSI({})", period),
                color,
                width: 1.5,
                style: LineStyleValue::Solid,
                points,
            }]),
            StudyOutput::Levels(levels),
        ]);
        Ok(StudyResult::ok())
    }
}
