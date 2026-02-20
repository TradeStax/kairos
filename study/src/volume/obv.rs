use crate::config::{ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::trend::sma::candle_key;
use data::SerializableColor;

const DEFAULT_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

pub struct ObvStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl ObvStudy {
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "color",
                label: "Color",
                description: "OBV line color",
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_COLOR),
            },
            ParameterDef {
                key: "width",
                label: "Width",
                description: "Line width",
                kind: ParameterKind::Float {
                    min: 0.5,
                    max: 5.0,
                    step: 0.5,
                },
                default: ParameterValue::Float(1.5),
            },
        ];

        let mut config = StudyConfig::new("obv");
        for p in &params {
            config.set(p.key, p.default.clone());
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

    fn set_parameter(&mut self, key: &str, value: ParameterValue) -> Result<(), StudyError> {
        if !self.params.iter().any(|p| p.key == key) {
            return Err(StudyError::InvalidParameter {
                key: key.to_string(),
                reason: "unknown parameter".to_string(),
            });
        }
        self.config.set(key, value);
        Ok(())
    }

    fn compute(&mut self, input: &StudyInput) {
        let color = self.config.get_color("color", DEFAULT_COLOR);
        let width = self.config.get_float("width", 1.5) as f32;

        let candles = input.candles;
        if candles.is_empty() {
            self.output = StudyOutput::Empty;
            return;
        }

        let mut obv: f64 = 0.0;
        let mut points = Vec::with_capacity(candles.len());

        // First candle: OBV starts at 0
        let key = candle_key(&candles[0], 0, &input.basis);
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

            let key = candle_key(&candles[i], i, &input.basis);
            points.push((key, obv as f32));
        }

        self.output = StudyOutput::Lines(vec![LineSeries {
            label: "OBV".to_string(),
            color,
            width,
            style: crate::config::LineStyleValue::Solid,
            points,
        }]);
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
        study.compute(&input);
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_obv_single_candle() {
        let mut study = ObvStudy::new();
        let candles = vec![make_candle(1000, 100.0, 50.0, 50.0)];
        let input = make_input(&candles);
        study.compute(&input);

        if let StudyOutput::Lines(lines) = study.output() {
            assert_eq!(lines[0].points.len(), 1);
            assert!((lines[0].points[0].1).abs() < 0.01);
        } else {
            panic!("expected Lines output");
        }
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
        study.compute(&input);

        if let StudyOutput::Lines(lines) = study.output() {
            let pts = &lines[0].points;
            assert_eq!(pts.len(), 5);
            assert!((pts[0].1 - 0.0).abs() < 0.01);
            assert!((pts[1].1 - 100.0).abs() < 0.01);
            assert!((pts[2].1 - 20.0).abs() < 0.01);
            assert!((pts[3].1 - 20.0).abs() < 0.01); // unchanged
            assert!((pts[4].1 - 120.0).abs() < 0.01);
        } else {
            panic!("expected Lines output");
        }
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
        study.compute(&input);

        if let StudyOutput::Lines(lines) = study.output() {
            let pts = &lines[0].points;
            assert!((pts[0].1).abs() < 0.01);
            assert!((pts[1].1 - (-100.0)).abs() < 0.01);
            assert!((pts[2].1 - (-200.0)).abs() < 0.01);
        } else {
            panic!("expected Lines output");
        }
    }
}
