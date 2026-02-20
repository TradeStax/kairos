//! Volume Profile Study
//!
//! Displays the distribution of trading volume across price levels
//! for visible candles. Identifies POC and Value Area.

use crate::config::{ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{ProfileData, ProfileLevel, ProfileSide, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use data::SerializableColor;
use std::collections::BTreeMap;

const DEFAULT_WIDTH_PCT: f64 = 0.3;

const DEFAULT_POC_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 0.84,
    b: 0.0,
    a: 1.0,
};

const DEFAULT_VAL_COLOR: SerializableColor = SerializableColor {
    r: 0.3,
    g: 0.6,
    b: 1.0,
    a: 0.5,
};

const DEFAULT_VAR_COLOR: SerializableColor = SerializableColor {
    r: 0.5,
    g: 0.5,
    b: 0.5,
    a: 0.3,
};

pub struct VolumeProfileStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl VolumeProfileStudy {
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "width_pct",
                label: "Width %",
                description: "Profile width as percentage of chart",
                kind: ParameterKind::Float {
                    min: 0.05,
                    max: 0.5,
                    step: 0.05,
                },
                default: ParameterValue::Float(DEFAULT_WIDTH_PCT),
            },
            ParameterDef {
                key: "poc_color",
                label: "POC Color",
                description: "Point of Control highlight color",
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_POC_COLOR),
            },
            ParameterDef {
                key: "val_color",
                label: "Value Area Color",
                description: "Value Area fill color",
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_VAL_COLOR),
            },
            ParameterDef {
                key: "var_color",
                label: "Volume Area Color",
                description: "Volume bars outside Value Area",
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_VAR_COLOR),
            },
        ];

        let mut config = StudyConfig::new("volume_profile");
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

impl Default for VolumeProfileStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a volume profile from candle data, returning sorted price levels.
fn build_profile_from_candles(
    candles: &[data::Candle],
    tick_size: data::Price,
) -> Vec<ProfileLevel> {
    let mut volume_map: BTreeMap<i64, (f64, f64)> = BTreeMap::new();

    for c in candles {
        // Distribute the candle's volume evenly across price levels
        // from low to high at tick_size increments
        let low_units = c.low.round_to_tick(tick_size).units();
        let high_units = c.high.round_to_tick(tick_size).units();
        let step = tick_size.units();

        if step <= 0 || high_units < low_units {
            continue;
        }

        let num_levels = ((high_units - low_units) / step + 1) as f64;
        if num_levels <= 0.0 {
            continue;
        }

        let buy_per_level = c.buy_volume.value() / num_levels;
        let sell_per_level = c.sell_volume.value() / num_levels;

        let mut price_units = low_units;
        while price_units <= high_units {
            let entry = volume_map.entry(price_units).or_insert((0.0, 0.0));
            entry.0 += buy_per_level;
            entry.1 += sell_per_level;
            price_units += step;
        }
    }

    volume_map
        .into_iter()
        .map(|(units, (buy, sell))| ProfileLevel {
            price: data::Price::from_units(units).to_f64(),
            buy_volume: buy as f32,
            sell_volume: sell as f32,
        })
        .collect()
}

/// Find the POC index (level with highest total volume).
fn find_poc_index(levels: &[ProfileLevel]) -> Option<usize> {
    levels
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            let total_a = a.buy_volume + a.sell_volume;
            let total_b = b.buy_volume + b.sell_volume;
            total_a
                .partial_cmp(&total_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
}

/// Calculate value area (70% of volume centered on POC).
/// Returns (vah_index, val_index).
fn calculate_value_area(
    levels: &[ProfileLevel],
    poc_idx: usize,
    percentage: f64,
) -> Option<(usize, usize)> {
    if levels.is_empty() {
        return None;
    }

    let total_volume: f32 = levels.iter().map(|l| l.buy_volume + l.sell_volume).sum();
    let target = total_volume * percentage as f32;

    let mut accumulated = levels[poc_idx].buy_volume + levels[poc_idx].sell_volume;
    let mut upper = poc_idx;
    let mut lower = poc_idx;

    while accumulated < target && (lower > 0 || upper < levels.len() - 1) {
        let up_vol = if upper + 1 < levels.len() {
            levels[upper + 1].buy_volume + levels[upper + 1].sell_volume
        } else {
            0.0
        };
        let down_vol = if lower > 0 {
            levels[lower - 1].buy_volume + levels[lower - 1].sell_volume
        } else {
            0.0
        };

        if up_vol >= down_vol && upper + 1 < levels.len() {
            upper += 1;
            accumulated += up_vol;
        } else if lower > 0 {
            lower -= 1;
            accumulated += down_vol;
        } else if upper + 1 < levels.len() {
            upper += 1;
            accumulated += up_vol;
        } else {
            break;
        }
    }

    Some((upper, lower))
}

impl Study for VolumeProfileStudy {
    fn id(&self) -> &str {
        "volume_profile"
    }

    fn name(&self) -> &str {
        "Volume Profile"
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
        if input.candles.is_empty() {
            self.output = StudyOutput::Empty;
            return;
        }

        let levels = build_profile_from_candles(input.candles, input.tick_size);

        if levels.is_empty() {
            self.output = StudyOutput::Empty;
            return;
        }

        let poc = find_poc_index(&levels);
        let value_area = poc.and_then(|poc_idx| calculate_value_area(&levels, poc_idx, 0.7));

        self.output = StudyOutput::Profile(ProfileData {
            side: ProfileSide::Left,
            levels,
            poc,
            value_area,
        });
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
    }

    #[test]
    fn test_build_profile() {
        let candles = vec![
            make_candle(1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0),
            make_candle(2000, 101.0, 103.0, 100.0, 102.0, 80.0, 60.0),
        ];

        let tick_size = Price::from_f32(1.0);
        let levels = build_profile_from_candles(&candles, tick_size);

        // Price range is 99 to 103 = 5 levels
        assert!(!levels.is_empty());
        assert!(levels.len() <= 5);

        // All volumes should be positive
        for level in &levels {
            assert!(level.buy_volume >= 0.0);
            assert!(level.sell_volume >= 0.0);
        }
    }

    #[test]
    fn test_find_poc() {
        let levels = vec![
            ProfileLevel {
                price: 99.0,
                buy_volume: 10.0,
                sell_volume: 5.0,
            },
            ProfileLevel {
                price: 100.0,
                buy_volume: 50.0,
                sell_volume: 40.0,
            },
            ProfileLevel {
                price: 101.0,
                buy_volume: 20.0,
                sell_volume: 10.0,
            },
        ];

        let poc = find_poc_index(&levels);
        assert_eq!(poc, Some(1)); // level 100.0 has highest total volume (90)
    }

    #[test]
    fn test_value_area() {
        let levels = vec![
            ProfileLevel {
                price: 98.0,
                buy_volume: 5.0,
                sell_volume: 5.0,
            },
            ProfileLevel {
                price: 99.0,
                buy_volume: 20.0,
                sell_volume: 10.0,
            },
            ProfileLevel {
                price: 100.0,
                buy_volume: 50.0,
                sell_volume: 40.0,
            },
            ProfileLevel {
                price: 101.0,
                buy_volume: 15.0,
                sell_volume: 15.0,
            },
            ProfileLevel {
                price: 102.0,
                buy_volume: 5.0,
                sell_volume: 5.0,
            },
        ];

        let poc_idx = 2; // price 100.0
        let va = calculate_value_area(&levels, poc_idx, 0.7);
        assert!(va.is_some());

        let (vah, val) = va.unwrap();
        // VAH should be >= POC, VAL should be <= POC
        assert!(vah >= poc_idx);
        assert!(val <= poc_idx);
    }

    #[test]
    fn test_volume_profile_compute() {
        let mut study = VolumeProfileStudy::new();
        let candles = vec![
            make_candle(1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0),
            make_candle(2000, 101.0, 103.0, 100.0, 102.0, 80.0, 60.0),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        };

        study.compute(&input);

        match &study.output {
            StudyOutput::Profile(data) => {
                assert!(!data.levels.is_empty());
                assert!(data.poc.is_some());
            }
            _ => panic!("Expected Profile output"),
        }
    }

    #[test]
    fn test_volume_profile_empty() {
        let mut study = VolumeProfileStudy::new();
        let candles: Vec<Candle> = vec![];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        };

        study.compute(&input);
        assert!(matches!(study.output(), StudyOutput::Empty));
    }
}
