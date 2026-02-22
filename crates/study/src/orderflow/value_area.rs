//! Value Area Study
//!
//! Identifies the Value Area High (VAH) and Value Area Low (VAL) which
//! represent the price range containing a specified percentage of volume.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterTab,
    ParameterValue, StudyConfig, Visibility,
};
use crate::error::StudyError;
use crate::orderflow::profile_core;
use crate::output::{LineSeries, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use data::SerializableColor;

const DEFAULT_PERCENTAGE: f64 = 0.7;

const DEFAULT_VAH_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.8,
    b: 0.4,
    a: 0.8,
};

const DEFAULT_VAL_COLOR: SerializableColor = SerializableColor {
    r: 0.9,
    g: 0.2,
    b: 0.2,
    a: 0.8,
};

const DEFAULT_FILL_OPACITY: f64 = 0.1;

pub struct ValueAreaStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl ValueAreaStudy {
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "percentage".into(),
                label: "Percentage".into(),
                description: "Volume percentage for value area (0.5-0.95)".into(),
                kind: ParameterKind::Float {
                    min: 0.5,
                    max: 0.95,
                    step: 0.05,
                },
                default: ParameterValue::Float(DEFAULT_PERCENTAGE),
                tab: ParameterTab::Parameters,
                section: None,
                order: 0,
                format: DisplayFormat::Percent,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "vah_color".into(),
                label: "VAH Color".into(),
                description: "Value Area High line color".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_VAH_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "val_color".into(),
                label: "VAL Color".into(),
                description: "Value Area Low line color".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_VAL_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "fill_opacity".into(),
                label: "Fill Opacity".into(),
                description: "Fill opacity between VAH and VAL".into(),
                kind: ParameterKind::Float {
                    min: 0.0,
                    max: 0.5,
                    step: 0.05,
                },
                default: ParameterValue::Float(DEFAULT_FILL_OPACITY),
                tab: ParameterTab::Style,
                section: None,
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
        ];

        let mut config = StudyConfig::new("value_area");
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

impl Default for ValueAreaStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a volume profile and compute value area from candles.
/// Returns (vah_price, val_price) or None.
fn compute_value_area(
    candles: &[data::Candle],
    tick_size: data::Price,
    percentage: f64,
) -> Option<(f64, f64)> {
    if candles.is_empty() {
        return None;
    }

    let levels =
        profile_core::build_profile_from_candles(candles, tick_size, tick_size.units());
    if levels.is_empty() {
        return None;
    }

    let poc_idx = profile_core::find_poc_index(&levels)?;
    let (vah_idx, val_idx) =
        profile_core::calculate_value_area(&levels, poc_idx, percentage)?;

    Some((levels[vah_idx].price, levels[val_idx].price))
}

impl Study for ValueAreaStudy {
    fn id(&self) -> &str {
        "value_area"
    }

    fn name(&self) -> &str {
        "Value Area"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::OrderFlow
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Background
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
        let percentage = self.config.get_float("percentage", DEFAULT_PERCENTAGE);
        let vah_color = self.config.get_color("vah_color", DEFAULT_VAH_COLOR);
        let val_color = self.config.get_color("val_color", DEFAULT_VAL_COLOR);
        let fill_opacity = self.config.get_float("fill_opacity", DEFAULT_FILL_OPACITY) as f32;

        if input.candles.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        match compute_value_area(input.candles, input.tick_size, percentage) {
            Some((vah, val)) => {
                // Create constant lines spanning all candle times
                let times: Vec<u64> = input.candles.iter().map(|c| c.time.to_millis()).collect();

                let upper_points: Vec<(u64, f32)> =
                    times.iter().map(|&t| (t, vah as f32)).collect();
                let lower_points: Vec<(u64, f32)> =
                    times.iter().map(|&t| (t, val as f32)).collect();

                self.output = StudyOutput::Band {
                    upper: LineSeries {
                        label: "VAH".to_string(),
                        color: vah_color,
                        width: 1.0,
                        style: LineStyleValue::Dashed,
                        points: upper_points,
                    },
                    lower: LineSeries {
                        label: "VAL".to_string(),
                        color: val_color,
                        width: 1.0,
                        style: LineStyleValue::Dashed,
                        points: lower_points,
                    },
                    middle: None,
                    fill_opacity,
                };
            }
            None => {
                self.output = StudyOutput::Empty;
            }
        }
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
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        buy_vol: f64,
        sell_vol: f64,
    ) -> Candle {
        Candle::new(
            Timestamp::from_millis(time),
            Price::from_f32(open),
            Price::from_f32(high),
            Price::from_f32(low),
            Price::from_f32(close),
            Volume(buy_vol),
            Volume(sell_vol),
        )
        .expect("test: valid candle")
    }

    #[test]
    fn test_value_area_basic() {
        let mut study = ValueAreaStudy::new();
        let candles = vec![
            make_candle(1000, 100.0, 105.0, 95.0, 102.0, 200.0, 150.0),
            make_candle(2000, 102.0, 106.0, 98.0, 104.0, 180.0, 120.0),
            make_candle(3000, 104.0, 107.0, 99.0, 103.0, 160.0, 140.0),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        };

        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Band {
                upper,
                lower,
                middle,
                fill_opacity,
            } => {
                assert!(!upper.points.is_empty());
                assert!(!lower.points.is_empty());
                assert!(middle.is_none());
                assert!(*fill_opacity > 0.0);
                // VAH should be above VAL
                let vah = upper.points[0].1;
                let val = lower.points[0].1;
                assert!(vah >= val);
            }
            other => assert!(matches!(other, StudyOutput::Band { .. }), "Expected Band output"),
        }
    }

    #[test]
    fn test_value_area_empty() {
        let mut study = ValueAreaStudy::new();
        let candles: Vec<Candle> = vec![];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        };

        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_value_area_computation() {
        // Test the raw computation
        let candles = vec![make_candle(1000, 100.0, 110.0, 90.0, 105.0, 500.0, 500.0)];

        let result = compute_value_area(&candles, Price::from_f32(1.0), 0.7);
        assert!(result.is_some());

        let (vah, val) = result.unwrap();
        // VAH should be within the candle range
        assert!(vah >= 90.0 && vah <= 110.0);
        assert!(val >= 90.0 && val <= 110.0);
        assert!(vah >= val);
    }
}
