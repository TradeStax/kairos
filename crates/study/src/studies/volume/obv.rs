//! On Balance Volume (OBV).
//!
//! Cumulative indicator: adds the candle's volume when the close is higher
//! than the previous close, subtracts it when lower.
//! Formula: `OBV(t) = OBV(t-1) + sign(Close(t) - Close(t-1)) * Volume(t)`
//!
//! Output: `StudyOutput::Lines` — a single cumulative line.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::util::candle_key;
use data::SerializableColor;

const DEFAULT_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

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
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl ObvStudy {
    /// Create a new OBV study with a white line at 1.5px width.
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "color".into(),
                label: "Color".into(),
                description: "OBV line color".into(),
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
        ];

        let mut config = StudyConfig::new("obv");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }

        Self {
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
    fn id(&self) -> &str {
        "obv"
    }

    fn name(&self) -> &str {
        "On Balance Volume"
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

        let candles = input.candles;
        if candles.is_empty() {
            log::debug!("{}: no candle data", self.id());
            self.output = StudyOutput::Empty;
            return Ok(());
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

    fn make_candle(time: u64, close: f32, buy_vol: f64, sell_vol: f64) -> Candle {
        Candle::new(
            Timestamp(time),
            Price::from_f32(close),
            Price::from_f32(close + 1.0),
            Price::from_f32(close - 1.0),
            Price::from_f32(close),
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
    fn test_obv_empty() {
        let mut study = ObvStudy::new();
        let input = make_input(&[]);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_obv_single_candle() {
        let mut study = ObvStudy::new();
        let candles = vec![make_candle(1000, 100.0, 50.0, 50.0)];
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
        assert_eq!(lines[0].points.len(), 1);
        assert!((lines[0].points[0].1).abs() < 0.01);
    }

    #[test]
    fn test_obv_calculation() {
        let mut study = ObvStudy::new();
        let candles = vec![
            make_candle(1000, 100.0, 50.0, 50.0), // OBV = 0
            make_candle(2000, 105.0, 60.0, 40.0), // close up, vol=100 => OBV = +100
            make_candle(3000, 102.0, 30.0, 50.0), // close down, vol=80 => OBV = +20
            make_candle(4000, 102.0, 45.0, 45.0), // close equal, vol=90 => OBV = +20
            make_candle(5000, 110.0, 70.0, 30.0), // close up, vol=100 => OBV = +120
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
        assert_eq!(pts.len(), 5);
        assert!((pts[0].1 - 0.0).abs() < 0.01);
        assert!((pts[1].1 - 100.0).abs() < 0.01);
        assert!((pts[2].1 - 20.0).abs() < 0.01);
        assert!((pts[3].1 - 20.0).abs() < 0.01); // unchanged
        assert!((pts[4].1 - 120.0).abs() < 0.01);
    }

    #[test]
    fn test_obv_downtrend() {
        let mut study = ObvStudy::new();
        let candles = vec![
            make_candle(1000, 100.0, 50.0, 50.0),
            make_candle(2000, 95.0, 40.0, 60.0), // down, vol=100 => -100
            make_candle(3000, 90.0, 30.0, 70.0), // down, vol=100 => -200
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
        assert!((pts[0].1).abs() < 0.01);
        assert!((pts[1].1 - (-100.0)).abs() < 0.01);
        assert!((pts[2].1 - (-200.0)).abs() < 0.01);
    }

    #[test]
    fn obv_zero_volume_candles_unchanged() {
        let mut study = ObvStudy::new();
        let candles = vec![
            make_candle(1000, 100.0, 50.0, 50.0),
            // Close up but zero volume -> OBV should still add 0
            Candle::new(
                Timestamp(2000),
                Price::from_f32(105.0),
                Price::from_f32(106.0),
                Price::from_f32(104.0),
                Price::from_f32(105.0),
                Volume(0.0),
                Volume(0.0),
            )
            .expect("valid candle"),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let StudyOutput::Lines(lines) = study.output() else {
            panic!("expected Lines output")
        };
        let pts = &lines[0].points;
        assert!((pts[0].1).abs() < 0.01); // first candle OBV = 0
        assert!((pts[1].1).abs() < 0.01); // close up, but vol=0 => 0+0=0
    }

    #[test]
    fn obv_all_equal_closes() {
        let mut study = ObvStudy::new();
        let candles = vec![
            make_candle(1000, 100.0, 50.0, 50.0),
            make_candle(2000, 100.0, 60.0, 40.0), // equal close, vol=100 => unchanged
            make_candle(3000, 100.0, 70.0, 30.0), // equal close, vol=100 => unchanged
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let StudyOutput::Lines(lines) = study.output() else {
            panic!("expected Lines output")
        };
        let pts = &lines[0].points;
        // All closes equal => OBV stays at 0 throughout
        assert!((pts[0].1).abs() < 0.01);
        assert!((pts[1].1).abs() < 0.01);
        assert!((pts[2].1).abs() < 0.01);
    }

    #[test]
    fn obv_reset_clears_output() {
        let mut study = ObvStudy::new();
        let candles = vec![make_candle(1000, 100.0, 50.0, 50.0)];
        let input = make_input(&candles);
        study.compute(&input).unwrap();
        assert!(!matches!(study.output(), StudyOutput::Empty));

        study.reset();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn obv_alternating_direction() {
        let mut study = ObvStudy::new();
        let candles = vec![
            make_candle(1000, 100.0, 50.0, 50.0), // OBV = 0
            make_candle(2000, 110.0, 40.0, 60.0), // up, vol=100 => +100
            make_candle(3000, 105.0, 30.0, 70.0), // down, vol=100 => 0
            make_candle(4000, 115.0, 80.0, 20.0), // up, vol=100 => +100
            make_candle(5000, 108.0, 25.0, 75.0), // down, vol=100 => 0
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let StudyOutput::Lines(lines) = study.output() else {
            panic!("expected Lines output")
        };
        let pts = &lines[0].points;
        assert_eq!(pts.len(), 5);
        assert!((pts[0].1 - 0.0).abs() < 0.01);
        assert!((pts[1].1 - 100.0).abs() < 0.01);
        assert!((pts[2].1 - 0.0).abs() < 0.01);
        assert!((pts[3].1 - 100.0).abs() < 0.01);
        assert!((pts[4].1 - 0.0).abs() < 0.01);
    }
}
