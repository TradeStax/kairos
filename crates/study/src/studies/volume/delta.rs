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
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, StudyConfig,
    Visibility,
};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{BarPoint, BarSeries, StudyOutput};
use crate::util::candle_key;
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use data::SerializableColor;

const DEFAULT_POS_COLOR: SerializableColor = BULLISH_COLOR;

const DEFAULT_NEG_COLOR: SerializableColor = BEARISH_COLOR;

const DEFAULT_OPACITY: f64 = 0.8;

/// Per-candle volume delta (buy volume minus sell volume).
///
/// Positive delta means more contracts traded at the ask (aggressive
/// buyers); negative delta means more traded at the bid (aggressive
/// sellers). Renders as colored bars in a separate panel below the
/// price chart.
pub struct DeltaStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl DeltaStudy {
    /// Create a new Delta study with default bullish/bearish colors
    /// and 80% opacity.
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
            metadata: StudyMetadata {
                name: "Volume Delta".to_string(),
                category: StudyCategory::Volume,
                placement: StudyPlacement::Panel,
                description: "Buy minus sell volume per candle".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
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
        let pos_color = self.config.get_color("positive_color", DEFAULT_POS_COLOR);
        let neg_color = self.config.get_color("negative_color", DEFAULT_NEG_COLOR);
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
                let delta = c.volume_delta() as f32;
                let base_color = if delta >= 0.0 { pos_color } else { neg_color };
                BarPoint {
                    x: candle_key(c, i, total, &input.basis),
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
    fn test_delta_basic() {
        let mut study = DeltaStudy::new();
        let candles = vec![
            make_candle(1000, 300.0, 200.0), // delta = +100
            make_candle(2000, 100.0, 250.0), // delta = -150
            make_candle(3000, 200.0, 200.0), // delta = 0
        ];

        let input = make_input(&candles);

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
            other => assert!(
                matches!(other, StudyOutput::Bars(_)),
                "Expected Bars output"
            ),
        }
    }

    #[test]
    fn test_delta_empty() {
        let mut study = DeltaStudy::new();
        let candles: Vec<Candle> = vec![];
        let input = make_input(&candles);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn single_candle_delta() {
        let mut study = DeltaStudy::new();
        let candles = vec![make_candle(1000, 500.0, 200.0)]; // delta = +300
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::Bars(series) => {
                assert_eq!(series[0].points.len(), 1);
                assert!((series[0].points[0].value - 300.0).abs() < 1.0);
            }
            _ => panic!("Expected Bars output"),
        }
    }

    #[test]
    fn zero_volume_candle_produces_zero_delta() {
        let mut study = DeltaStudy::new();
        let candles = vec![make_candle(1000, 0.0, 0.0)]; // delta = 0
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::Bars(series) => {
                assert_eq!(series[0].points.len(), 1);
                assert!(series[0].points[0].value.abs() < 0.01);
                // Zero delta >= 0, so should use positive color (green-ish)
                assert!(series[0].points[0].color.g > series[0].points[0].color.r);
            }
            _ => panic!("Expected Bars output"),
        }
    }

    #[test]
    fn all_buy_volume_positive_delta() {
        let mut study = DeltaStudy::new();
        let candles = vec![make_candle(1000, 400.0, 0.0)]; // delta = +400
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::Bars(series) => {
                assert!((series[0].points[0].value - 400.0).abs() < 1.0);
                // Positive delta -> green
                assert!(series[0].points[0].color.g > series[0].points[0].color.r);
            }
            _ => panic!("Expected Bars output"),
        }
    }

    #[test]
    fn all_sell_volume_negative_delta() {
        let mut study = DeltaStudy::new();
        let candles = vec![make_candle(1000, 0.0, 350.0)]; // delta = -350
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::Bars(series) => {
                assert!((series[0].points[0].value - (-350.0)).abs() < 1.0);
                // Negative delta -> red
                assert!(series[0].points[0].color.r > series[0].points[0].color.g);
            }
            _ => panic!("Expected Bars output"),
        }
    }

    #[test]
    fn delta_reset_clears_output() {
        let mut study = DeltaStudy::new();
        let candles = vec![make_candle(1000, 300.0, 200.0)];
        let input = make_input(&candles);
        study.compute(&input).unwrap();
        assert!(!matches!(study.output(), StudyOutput::Empty));

        study.reset();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }
}
