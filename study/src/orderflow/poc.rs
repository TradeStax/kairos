//! Point of Control (POC) Study
//!
//! The Point of Control is the price level with the highest traded volume
//! within a rolling lookback window.

use crate::config::{LineStyleValue, ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use data::SerializableColor;
use std::collections::BTreeMap;

const DEFAULT_LOOKBACK: i64 = 20;

const DEFAULT_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 0.84,
    b: 0.0,
    a: 1.0,
};

const DEFAULT_WIDTH: f64 = 1.5;

pub struct PocStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl PocStudy {
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "lookback",
                label: "Lookback",
                description: "Number of candles for rolling POC",
                kind: ParameterKind::Integer { min: 1, max: 500 },
                default: ParameterValue::Integer(DEFAULT_LOOKBACK),
            },
            ParameterDef {
                key: "color",
                label: "Color",
                description: "POC line color",
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_COLOR),
            },
            ParameterDef {
                key: "width",
                label: "Width",
                description: "Line width in pixels",
                kind: ParameterKind::Float {
                    min: 0.5,
                    max: 5.0,
                    step: 0.5,
                },
                default: ParameterValue::Float(DEFAULT_WIDTH),
            },
        ];

        let mut config = StudyConfig::new("poc");
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

impl Default for PocStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the POC price from a set of candles by building a simple volume profile.
/// Distributes each candle's volume across its price range at tick_size increments,
/// then returns the price with the highest total volume.
fn find_poc_from_candles(candles: &[data::Candle], tick_size: data::Price) -> Option<f64> {
    let mut volume_map: BTreeMap<i64, f64> = BTreeMap::new();
    let step = tick_size.units();

    if step <= 0 {
        return None;
    }

    for c in candles {
        let low_units = c.low.round_to_tick(tick_size).units();
        let high_units = c.high.round_to_tick(tick_size).units();

        if high_units < low_units {
            continue;
        }

        let num_levels = ((high_units - low_units) / step + 1) as f64;
        if num_levels <= 0.0 {
            continue;
        }

        let vol_per_level = c.volume() as f64 / num_levels;

        let mut price_units = low_units;
        while price_units <= high_units {
            *volume_map.entry(price_units).or_insert(0.0) += vol_per_level;
            price_units += step;
        }
    }

    volume_map
        .into_iter()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(units, _)| data::Price::from_units(units).to_f64())
}

impl Study for PocStudy {
    fn id(&self) -> &str {
        "poc"
    }

    fn name(&self) -> &str {
        "Point of Control"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::OrderFlow
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

    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError> {
        let lookback = self.config.get_int("lookback", DEFAULT_LOOKBACK) as usize;
        let color = self.config.get_color("color", DEFAULT_COLOR);
        let width = self.config.get_float("width", DEFAULT_WIDTH) as f32;

        if input.candles.len() < lookback {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        let mut points = Vec::with_capacity(input.candles.len());

        for i in (lookback - 1)..input.candles.len() {
            let start = i + 1 - lookback;
            let window = &input.candles[start..=i];
            if let Some(poc_price) = find_poc_from_candles(window, input.tick_size) {
                points.push((input.candles[i].time.to_millis(), poc_price as f32));
            }
        }

        self.output = StudyOutput::Lines(vec![LineSeries {
            label: "POC".to_string(),
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

    fn make_candle(time: u64, open: f32, high: f32, low: f32, close: f32, vol: f64) -> Candle {
        Candle::new(
            Timestamp::from_millis(time),
            Price::from_f32(open),
            Price::from_f32(high),
            Price::from_f32(low),
            Price::from_f32(close),
            Volume(vol * 0.6),
            Volume(vol * 0.4),
        )
    }

    #[test]
    fn test_poc_basic() {
        let mut study = PocStudy::new();
        study
            .set_parameter("lookback", ParameterValue::Integer(3))
            .unwrap();

        let candles = vec![
            make_candle(1000, 100.0, 102.0, 99.0, 101.0, 100.0),
            make_candle(2000, 101.0, 103.0, 100.0, 102.0, 200.0),
            make_candle(3000, 102.0, 104.0, 101.0, 103.0, 150.0),
            make_candle(4000, 103.0, 105.0, 102.0, 104.0, 80.0),
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
            StudyOutput::Lines(lines) => {
                assert_eq!(lines.len(), 1);
                // With lookback=3 and 4 candles, we should get 2 POC points
                assert_eq!(lines[0].points.len(), 2);
                // POC price should be within the price range of the candles
                for (_, price) in &lines[0].points {
                    assert!(*price >= 99.0 && *price <= 105.0);
                }
            }
            other => assert!(matches!(other, StudyOutput::Lines(_)), "Expected Lines output"),
        }
    }

    #[test]
    fn test_poc_insufficient_data() {
        let mut study = PocStudy::new();
        let candles = vec![make_candle(1000, 100.0, 102.0, 99.0, 101.0, 100.0)];

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
}
