use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, StudyConfig,
    Visibility,
};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
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

pub struct CvdStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl CvdStudy {
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
            config,
            output: StudyOutput::Empty,
            params,
        }
    }

    /// Check if we should reset the cumulative sum based on the reset period.
    /// Returns true if the candle crosses a period boundary relative to the
    /// previous candle.
    fn should_reset(&self, prev_millis: u64, curr_millis: u64, reset_period: &str) -> bool {
        match reset_period {
            "Daily" => {
                let prev_secs = (prev_millis / 1000) as i64;
                let curr_secs = (curr_millis / 1000) as i64;
                let prev_dt = chrono::DateTime::from_timestamp(prev_secs, 0);
                let curr_dt = chrono::DateTime::from_timestamp(curr_secs, 0);
                match (prev_dt, curr_dt) {
                    (Some(p), Some(c)) => p.date_naive() != c.date_naive(),
                    _ => false,
                }
            }
            "Weekly" => {
                // Reset when the ISO week changes (weeks start Monday)
                use chrono::Datelike;
                let prev_secs = (prev_millis / 1000) as i64;
                let curr_secs = (curr_millis / 1000) as i64;
                let prev_dt = chrono::DateTime::from_timestamp(prev_secs, 0);
                let curr_dt = chrono::DateTime::from_timestamp(curr_secs, 0);
                match (prev_dt, curr_dt) {
                    (Some(p), Some(c)) => {
                        let prev_week = (p.iso_week().year(), p.iso_week().week());
                        let curr_week = (c.iso_week().year(), c.iso_week().week());
                        curr_week != prev_week
                    }
                    _ => false,
                }
            }
            _ => false,
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

    fn name(&self) -> &str {
        "Cumulative Volume Delta"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::Volume
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Panel
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

    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError> {
        let color = self.config.get_color("color", DEFAULT_COLOR);
        let width = self.config.get_float("width", 1.5) as f32;
        let reset_period = self.config.get_choice("reset_period", "None").to_string();

        let candles = input.candles;
        if candles.is_empty() {
            log::debug!("{}: no candle data", self.id());
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        let mut cum_delta: f64 = 0.0;
        let mut points = Vec::with_capacity(candles.len());

        for (i, candle) in candles.iter().enumerate() {
            // Check for period reset
            if i > 0 && reset_period != "None" {
                let prev_time = candles[i - 1].time.to_millis();
                let curr_time = candle.time.to_millis();
                if self.should_reset(prev_time, curr_time, &reset_period) {
                    cum_delta = 0.0;
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
            style: crate::config::LineStyleValue::Solid,
            points,
        }]);
        Ok(())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(Self {
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{Candle, ChartBasis, Price, Timeframe, Timestamp, Volume};

    fn make_candle(time: u64, buy_vol: f64, sell_vol: f64) -> Candle {
        Candle::new(
            Timestamp::from_millis(time),
            Price::from_f32(100.0),
            Price::from_f32(102.0),
            Price::from_f32(99.0),
            Price::from_f32(101.0),
            Volume(buy_vol),
            Volume(sell_vol),
        )
        .expect("test: valid candle")
    }

    fn make_input(candles: &[Candle]) -> StudyInput<'_> {
        StudyInput {
            candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        }
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
}
