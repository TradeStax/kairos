//! Cumulative Volume Delta (CVD).
//!
//! Running sum of per-candle delta (buy volume minus sell volume). A
//! rising CVD line indicates sustained buying pressure; a falling line
//! indicates sustained selling pressure.
//!
//! Divergences between CVD and price are a key signal: price making new
//! highs while CVD trends lower suggests weakening demand, and vice versa.
//!
//! Supports optional daily or weekly resets to isolate intraday or
//! intraweek order flow patterns.
//!
//! Output: `StudyOutput::Lines` — a single cumulative line.

mod params;
#[cfg(test)]
mod tests;

use crate::config::LineStyleValue;
use crate::config::{ParameterValue, StudyConfig};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::util::candle_key;
use params::DEFAULT_COLOR;

/// Cumulative Volume Delta line study.
///
/// Maintains a running sum of per-candle delta (buy minus sell volume).
/// A rising CVD line confirms buying pressure behind a price advance;
/// divergence (price rising while CVD falls) warns of weakening demand.
///
/// Renders as a single line in a separate panel. Supports optional
/// daily or weekly resets via the `reset_period` parameter so that
/// intraday or intraweek order flow can be analysed in isolation.
pub struct CvdStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<crate::config::ParameterDef>,
}

impl CvdStudy {
    /// Create a new CVD study with a blue line, 1.5px width, and no
    /// reset period (cumulates across the entire visible range).
    pub fn new() -> Self {
        let params = params::make_params();
        let mut config = StudyConfig::from_params("cvd", &params);
        config.set("reset_period", ParameterValue::Choice("None".to_string()));

        Self {
            metadata: StudyMetadata {
                name: "Cumulative Volume Delta".to_string(),
                category: StudyCategory::Volume,
                placement: StudyPlacement::Panel,
                description: "Cumulative sum of buy minus sell volume".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for CvdStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for CvdStudy {
    crate::impl_study_base!("cvd");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let color = self.config.get_color("color", DEFAULT_COLOR);
        let width = self.config.get_float("width", 1.5) as f32;
        let reset_period = self.config.get_choice("reset_period", "None").to_string();

        let candles = input.candles;
        if candles.is_empty() {
            log::debug!("{}: no candle data", self.id());
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        let mut cum_delta: f64 = 0.0;
        let mut points = Vec::with_capacity(candles.len());

        // Cache the previous candle's date/week to avoid redundant
        // chrono::DateTime::from_timestamp() calls. Each candle's
        // "current" becomes the next candle's "previous", halving
        // the number of datetime conversions.
        let needs_reset = reset_period != "None";
        let is_daily = reset_period == "Daily";

        // prev_day: cached NaiveDate of the previous candle
        // prev_week: cached (iso_year, iso_week) of the previous candle
        let mut prev_day: Option<chrono::NaiveDate> = None;
        let mut prev_week: Option<(i32, u32)> = None;

        for (i, candle) in candles.iter().enumerate() {
            if needs_reset {
                let curr_secs = (candle.time.to_millis() / 1000) as i64;
                if let Some(curr_dt) = chrono::DateTime::from_timestamp(curr_secs, 0) {
                    if is_daily {
                        let curr_date = curr_dt.date_naive();
                        if prev_day.is_some_and(|pd| curr_date != pd) {
                            cum_delta = 0.0;
                        }
                        prev_day = Some(curr_date);
                    } else {
                        // Weekly
                        use chrono::Datelike;
                        let curr_wk = (curr_dt.iso_week().year(), curr_dt.iso_week().week());
                        if prev_week.is_some_and(|pw| curr_wk != pw) {
                            cum_delta = 0.0;
                        }
                        prev_week = Some(curr_wk);
                    }
                }
            }

            let delta = candle.volume_delta();
            cum_delta += delta;

            let key = candle_key(candle, i, candles.len(), &input.basis);
            points.push((key, cum_delta as f32));
        }

        self.output = StudyOutput::Lines(vec![LineSeries {
            label: "CVD".to_string(),
            color,
            width,
            style: LineStyleValue::Solid,
            points,
        }]);
        Ok(StudyResult::ok())
    }
}
