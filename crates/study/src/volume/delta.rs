//! Volume Delta.
//!
//! Per-candle delta: buy volume minus sell volume. Positive delta indicates
//! net buying pressure; negative delta indicates net selling pressure.
//!
//! Requires trade-level data (`StudyInput::trades`). Returns `StudyOutput::Empty`
//! if no trade data is available.
//!
//! Output: `StudyOutput::Bars` — one bar per candle, colored by sign.

use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};
use crate::error::StudyError;
use crate::output::{BarPoint, BarSeries, StudyOutput};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use data::SerializableColor;

const DEFAULT_POS_COLOR: SerializableColor = BULLISH_COLOR;

const DEFAULT_NEG_COLOR: SerializableColor = BEARISH_COLOR;

const DEFAULT_OPACITY: f64 = 0.8;

pub struct DeltaStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl DeltaStudy {
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "positive_color".into(),
                label: "Positive Color".into(),
                description: "Color for positive delta bars".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_POS_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "negative_color".into(),
                label: "Negative Color".into(),
                description: "Color for negative delta bars".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_NEG_COLOR),
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

        let mut config = StudyConfig::new("delta");
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

impl Default for DeltaStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for DeltaStudy {
    fn id(&self) -> &str {
        "delta"
    }

    fn name(&self) -> &str {
        "Volume Delta"
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
        let pos_color = self.config.get_color("positive_color", DEFAULT_POS_COLOR);
        let neg_color = self.config.get_color("negative_color", DEFAULT_NEG_COLOR);
        let opacity = self.config.get_float("opacity", DEFAULT_OPACITY) as f32;

        let points: Vec<BarPoint> = input
            .candles
            .iter()
            .map(|c| {
                let delta = c.volume_delta() as f32;
                let base_color = if delta >= 0.0 { pos_color } else { neg_color };
                BarPoint {
                    x: c.time.to_millis(),
                    value: delta,
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
            label: "Delta".to_string(),
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

    #[test]
    fn test_delta_basic() {
        let mut study = DeltaStudy::new();
        let candles = vec![
            make_candle(1000, 300.0, 200.0), // delta = +100
            make_candle(2000, 100.0, 250.0), // delta = -150
            make_candle(3000, 200.0, 200.0), // delta = 0
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
                let pts = &series[0].points;
                assert_eq!(pts.len(), 3);
                assert!((pts[0].value - 100.0).abs() < 1.0);
                assert!((pts[1].value - (-150.0)).abs() < 1.0);
                assert!((pts[2].value).abs() < 1.0);
                // Positive delta should be green-ish
                assert!(pts[0].color.g > pts[0].color.r);
                // Negative delta should be red-ish
                assert!(pts[1].color.r > pts[1].color.g);
            }
            other => assert!(matches!(other, StudyOutput::Bars(_)), "Expected Bars output"),
        }
    }

    #[test]
    fn test_delta_empty() {
        let mut study = DeltaStudy::new();
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
}
