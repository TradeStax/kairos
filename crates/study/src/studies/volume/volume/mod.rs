//! Basic Volume.
//!
//! Displays total volume per candle as colored bars. Green for bullish
//! candles (close >= open), red for bearish. This is the simplest volume
//! indicator and the most commonly used — it shows raw participation
//! levels at a glance.
//!
//! **Breakout confirmation**: a price breakout backed by above-average
//! volume is more likely to sustain. A breakout on thin volume is
//! suspect and often reverses.
//!
//! **Exhaustion / climax**: unusually high volume at a price extreme
//! (e.g. a parabolic move or sharp sell-off) may signal capitulation
//! and a pending reversal, especially when followed by declining volume.
//!
//! Output: `StudyOutput::Bars` — one bar per candle, colored by direction.

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
use params::{DEFAULT_DOWN_COLOR, DEFAULT_OPACITY, DEFAULT_UP_COLOR};

/// Basic volume bar chart.
///
/// Renders one bar per candle whose height is the total volume
/// (buy + sell) and whose color reflects the candle direction.
pub struct VolumeStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<crate::config::ParameterDef>,
}

impl VolumeStudy {
    /// Create a new Volume study with default colors and opacity.
    pub fn new() -> Self {
        let params = params::make_params();
        let config = StudyConfig::from_params("volume", &params);

        Self {
            metadata: StudyMetadata {
                name: "Volume".to_string(),
                category: StudyCategory::Volume,
                placement: StudyPlacement::Panel,
                description: "Total volume per candle".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for VolumeStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for VolumeStudy {
    crate::impl_study_base!("volume");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let up_color = self.config.get_color("up_color", DEFAULT_UP_COLOR);
        let down_color = self.config.get_color("down_color", DEFAULT_DOWN_COLOR);
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
                let is_bullish = c.close >= c.open;
                let base_color = if is_bullish { up_color } else { down_color };
                BarPoint {
                    x: candle_key(c, i, total, &input.basis),
                    value: c.volume(),
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
            label: "Volume".to_string(),
            points,
        }]);
        Ok(StudyResult::ok())
    }
}
