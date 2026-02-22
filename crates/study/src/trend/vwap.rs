use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, StudyConfig,
    Visibility,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::util::candle_key;
use data::SerializableColor;

const DEFAULT_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.9,
    b: 0.9,
    a: 1.0,
};

const BAND_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.9,
    b: 0.9,
    a: 0.4,
};

pub struct VwapStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl VwapStudy {
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "color".into(),
                label: "Color".into(),
                description: "VWAP line color".into(),
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
                key: "show_bands".into(),
                label: "Show Bands".into(),
                description: "Show standard deviation bands".into(),
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(false),
                tab: ParameterTab::Display,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "band_multiplier".into(),
                label: "Band Multiplier".into(),
                description: "Standard deviation multiplier for bands".into(),
                kind: ParameterKind::Float {
                    min: 1.0,
                    max: 3.0,
                    step: 0.5,
                },
                default: ParameterValue::Float(1.0),
                tab: ParameterTab::Parameters,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::WhenTrue("show_bands"),
            },
        ];

        let mut config = StudyConfig::new("vwap");
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

impl Default for VwapStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for VwapStudy {
    fn id(&self) -> &str {
        "vwap"
    }

    fn name(&self) -> &str {
        "Volume Weighted Average Price"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::Trend
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Overlay
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
        let show_bands = self.config.get_bool("show_bands", false);
        let band_mult = self.config.get_float("band_multiplier", 1.0);

        let candles = input.candles;
        if candles.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
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

    fn make_candle(
        time: u64,
        high: f32,
        low: f32,
        close: f32,
        buy_vol: f64,
        sell_vol: f64,
    ) -> Candle {
        let open = (high + low) / 2.0;
        Candle::new(
            Timestamp(time),
            Price::from_f32(open),
            Price::from_f32(high),
            Price::from_f32(low),
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
    fn test_vwap_empty() {
        let mut study = VwapStudy::new();
        let input = make_input(&[]);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_vwap_single_candle() {
        let mut study = VwapStudy::new();
        // TP = (30 + 10 + 20) / 3 = 20.0
        let candles = vec![make_candle(1000, 30.0, 10.0, 20.0, 50.0, 50.0)];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].points.len(), 1);
        assert!((lines[0].points[0].1 - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_vwap_calculation() {
        let mut study = VwapStudy::new();
        // Candle 1: TP = (12+8+10)/3 = 10, vol = 100
        // Candle 2: TP = (24+16+20)/3 = 20, vol = 200
        // VWAP after candle 1: 10*100/100 = 10.0
        // VWAP after candle 2: (10*100 + 20*200) / (100+200) = 5000/300 = 16.667
        let candles = vec![
            make_candle(1000, 12.0, 8.0, 10.0, 60.0, 40.0),
            make_candle(2000, 24.0, 16.0, 20.0, 120.0, 80.0),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        assert_eq!(lines.len(), 1);
        let pts = &lines[0].points;
        assert_eq!(pts.len(), 2);
        assert!((pts[0].1 - 10.0).abs() < 0.01);
        assert!((pts[1].1 - 16.667).abs() < 0.01);
    }

    #[test]
    fn test_vwap_with_bands() {
        let mut study = VwapStudy::new();
        study
            .set_parameter("show_bands", ParameterValue::Boolean(true))
            .unwrap();

        let candles = vec![
            make_candle(1000, 12.0, 8.0, 10.0, 60.0, 40.0),
            make_candle(2000, 24.0, 16.0, 20.0, 120.0, 80.0),
            make_candle(3000, 18.0, 12.0, 15.0, 80.0, 70.0),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].label, "VWAP");
        assert_eq!(lines[1].label, "VWAP Upper");
        assert_eq!(lines[2].label, "VWAP Lower");
        // Upper should be above VWAP, lower below
        for i in 0..lines[0].points.len() {
            assert!(lines[1].points[i].1 >= lines[0].points[i].1);
            assert!(lines[2].points[i].1 <= lines[0].points[i].1);
        }
    }

    #[test]
    fn test_vwap_zero_volume() {
        let mut study = VwapStudy::new();
        // Zero volume candle should use typical price as VWAP
        let candles = vec![make_candle(1000, 30.0, 10.0, 20.0, 0.0, 0.0)];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        assert_eq!(lines[0].points.len(), 1);
        // TP = (30+10+20)/3 = 20.0
        assert!((lines[0].points[0].1 - 20.0).abs() < 0.01);
    }
}
