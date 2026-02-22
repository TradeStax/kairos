//! Volume Study
//!
//! Displays total volume per candle as colored bars.
//! Green for bullish candles (close >= open), red for bearish.

use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};
use crate::error::StudyError;
use crate::output::{BarPoint, BarSeries, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::util::candle_key;
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use data::SerializableColor;

const DEFAULT_UP_COLOR: SerializableColor = BULLISH_COLOR;

const DEFAULT_DOWN_COLOR: SerializableColor = BEARISH_COLOR;

const DEFAULT_OPACITY: f64 = 0.8;

pub struct VolumeStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl VolumeStudy {
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "up_color".into(),
                label: "Up Color".into(),
                description: "Color for bullish volume bars".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_UP_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "down_color".into(),
                label: "Down Color".into(),
                description: "Color for bearish volume bars".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_DOWN_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "opacity".into(),
                label: "Opacity".into(),
                description: "Bar opacity".into(),
                kind: ParameterKind::Float {
                    min: 0.0,
                    max: 1.0,
                    step: 0.05,
                },
                default: ParameterValue::Float(DEFAULT_OPACITY),
                tab: ParameterTab::Style,
                section: None,
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
        ];

        let mut config = StudyConfig::new("volume");
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

impl Default for VolumeStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for VolumeStudy {
    fn id(&self) -> &str {
        "volume"
    }

    fn name(&self) -> &str {
        "Volume"
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
        let up_color = self.config.get_color("up_color", DEFAULT_UP_COLOR);
        let down_color = self.config.get_color("down_color", DEFAULT_DOWN_COLOR);
        let opacity = self.config.get_float("opacity", DEFAULT_OPACITY) as f32;

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

    fn make_candle(time: u64, open: f32, close: f32, vol: f64) -> Candle {
        let high = open.max(close) + 1.0;
        let low = open.min(close) - 1.0;
        Candle::new(
            Timestamp::from_millis(time),
            Price::from_f32(open),
            Price::from_f32(high),
            Price::from_f32(low),
            Price::from_f32(close),
            Volume(vol * 0.6),
            Volume(vol * 0.4),
        )
        .expect("test: valid candle")
    }

    #[test]
    fn test_volume_basic() {
        let mut study = VolumeStudy::new();
        let candles = vec![
            make_candle(1000, 100.0, 102.0, 500.0), // bullish
            make_candle(2000, 102.0, 99.0, 300.0),  // bearish
            make_candle(3000, 99.0, 101.0, 400.0),  // bullish
        ];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };

        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Bars(series) => {
                assert_eq!(series.len(), 1);
                assert_eq!(series[0].points.len(), 3);
                // Check volume values
                assert!((series[0].points[0].value - 500.0).abs() < 1.0);
                assert!((series[0].points[1].value - 300.0).abs() < 1.0);
                // Bullish bar should be green-ish
                assert!(series[0].points[0].color.g > series[0].points[0].color.r);
                // Bearish bar should be red-ish
                assert!(series[0].points[1].color.r > series[0].points[1].color.g);
            }
            other => assert!(matches!(other, StudyOutput::Bars(_)), "Expected Bars output"),
        }
    }

    #[test]
    fn test_volume_empty() {
        let mut study = VolumeStudy::new();
        let candles: Vec<Candle> = vec![];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };

        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Bars(series) => {
                assert_eq!(series[0].points.len(), 0);
            }
            other => assert!(matches!(other, StudyOutput::Bars(_)), "Expected Bars output"),
        }
    }

    #[test]
    fn test_volume_reset() {
        let mut study = VolumeStudy::new();
        let candles = vec![make_candle(1000, 100.0, 102.0, 500.0)];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };

        study.compute(&input).unwrap();
        assert!(!matches!(study.output(), StudyOutput::Empty));

        study.reset();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }
}
