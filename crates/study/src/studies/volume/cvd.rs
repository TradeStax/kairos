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

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::util::candle_key;
use data::SerializableColor;

const DEFAULT_COLOR: SerializableColor = SerializableColor {
    r: 0.3,
    g: 0.5,
    b: 1.0,
    a: 1.0,
};

const RESET_OPTIONS: &[&str] = &["None", "Daily", "Weekly"];

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
    params: Vec<ParameterDef>,
}

impl CvdStudy {
    /// Create a new CVD study with a blue line, 1.5px width, and no
    /// reset period (cumulates across the entire visible range).
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "color".into(),
                label: "Color".into(),
                description: "CVD line color".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "width".into(),
                label: "Width".into(),
                description: "Line width".into(),
                kind: ParameterKind::Float {
                    min: 0.5,
                    max: 5.0,
                    step: 0.5,
                },
                default: ParameterValue::Float(1.5),
                tab: ParameterTab::Style,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "reset_period".into(),
                label: "Reset Period".into(),
                description: "Reset cumulative delta at period boundaries".into(),
                kind: ParameterKind::Choice {
                    options: RESET_OPTIONS,
                },
                default: ParameterValue::Choice(String::new()),
                tab: ParameterTab::Parameters,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
        ];

        let mut config = StudyConfig::new("cvd");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }
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
    fn id(&self) -> &str {
        "cvd"
    }

    fn metadata(&self) -> &StudyMetadata {
        &self.metadata
    }

    fn parameters(&self) -> &[ParameterDef] {
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

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

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(Self {
            metadata: self.metadata.clone(),
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::test_helpers::{make_candle_ohlcv, make_input};
    use data::Candle;

    fn make_candle(time: u64, buy_vol: f64, sell_vol: f64) -> Candle {
        make_candle_ohlcv(time, 100.0, 102.0, 99.0, 101.0, buy_vol, sell_vol)
    }

    #[test]
    fn test_cvd_empty() {
        let mut study = CvdStudy::new();
        let input = make_input(&[]);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_cvd_cumulative() {
        let mut study = CvdStudy::new();
        let candles = vec![
            make_candle(1000, 300.0, 200.0), // delta = +100
            make_candle(2000, 100.0, 250.0), // delta = -150
            make_candle(3000, 200.0, 150.0), // delta = +50
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Lines(_)),
            "expected Lines output"
        );
        let StudyOutput::Lines(lines) = output else {
            unreachable!()
        };
        assert_eq!(lines.len(), 1);
        let pts = &lines[0].points;
        assert_eq!(pts.len(), 3);
        assert!((pts[0].1 - 100.0).abs() < 1.0);
        assert!((pts[1].1 - (-50.0)).abs() < 1.0);
        assert!((pts[2].1 - 0.0).abs() < 1.0);
    }

    #[test]
    fn test_cvd_daily_reset() {
        let mut study = CvdStudy::new();
        study
            .set_parameter("reset_period", ParameterValue::Choice("Daily".to_string()))
            .unwrap();

        // Day 1: 2 candles
        let day1_start = 86_400_000u64; // start of day 1
        // Day 2: 1 candle
        let day2_start = 86_400_000u64 * 2;

        let candles = vec![
            make_candle(day1_start, 300.0, 200.0),          // +100
            make_candle(day1_start + 60_000, 200.0, 100.0), // +100 => cum 200
            make_candle(day2_start, 100.0, 300.0),          // -200, reset => -200
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Lines(_)),
            "expected Lines output"
        );
        let StudyOutput::Lines(lines) = output else {
            unreachable!()
        };
        let pts = &lines[0].points;
        assert_eq!(pts.len(), 3);
        assert!((pts[0].1 - 100.0).abs() < 1.0);
        assert!((pts[1].1 - 200.0).abs() < 1.0);
        // Day boundary reset, so cumulative starts fresh
        assert!((pts[2].1 - (-200.0)).abs() < 1.0);
    }

    #[test]
    fn test_cvd_no_reset() {
        let mut study = CvdStudy::new();

        let day1_start = 86_400_000u64;
        let day2_start = 86_400_000u64 * 2;

        let candles = vec![
            make_candle(day1_start, 300.0, 200.0),
            make_candle(day2_start, 100.0, 300.0),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Lines(_)),
            "expected Lines output"
        );
        let StudyOutput::Lines(lines) = output else {
            unreachable!()
        };
        let pts = &lines[0].points;
        // No reset, so cumulative continues
        assert!((pts[0].1 - 100.0).abs() < 1.0);
        assert!((pts[1].1 - (-100.0)).abs() < 1.0);
    }

    #[test]
    fn single_candle_cvd_equals_delta() {
        let mut study = CvdStudy::new();
        let candles = vec![make_candle(1000, 500.0, 200.0)]; // delta = +300
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let StudyOutput::Lines(lines) = study.output() else {
            panic!("expected Lines output")
        };
        assert_eq!(lines[0].points.len(), 1);
        assert!((lines[0].points[0].1 - 300.0).abs() < 1.0);
    }

    #[test]
    fn zero_volume_candles_cvd_stays_flat() {
        let mut study = CvdStudy::new();
        let candles = vec![
            make_candle(1000, 100.0, 50.0), // delta = +50
            make_candle(2000, 0.0, 0.0),    // delta = 0
            make_candle(3000, 0.0, 0.0),    // delta = 0
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let StudyOutput::Lines(lines) = study.output() else {
            panic!("expected Lines output")
        };
        let pts = &lines[0].points;
        assert!((pts[0].1 - 50.0).abs() < 1.0);
        assert!((pts[1].1 - 50.0).abs() < 1.0); // unchanged
        assert!((pts[2].1 - 50.0).abs() < 1.0); // unchanged
    }

    #[test]
    fn cvd_all_negative_deltas() {
        let mut study = CvdStudy::new();
        let candles = vec![
            make_candle(1000, 100.0, 200.0), // delta = -100
            make_candle(2000, 50.0, 300.0),  // delta = -250
            make_candle(3000, 0.0, 100.0),   // delta = -100
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let StudyOutput::Lines(lines) = study.output() else {
            panic!("expected Lines output")
        };
        let pts = &lines[0].points;
        assert!((pts[0].1 - (-100.0)).abs() < 1.0);
        assert!((pts[1].1 - (-350.0)).abs() < 1.0);
        assert!((pts[2].1 - (-450.0)).abs() < 1.0);
    }

    #[test]
    fn cvd_reset_clears_output() {
        let mut study = CvdStudy::new();
        let candles = vec![make_candle(1000, 200.0, 100.0)];
        let input = make_input(&candles);
        study.compute(&input).unwrap();
        assert!(!matches!(study.output(), StudyOutput::Empty));

        study.reset();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }
}
