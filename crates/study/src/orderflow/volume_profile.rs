//! Volume Profile Study
//!
//! Displays the distribution of trading volume across price levels
//! for visible candles. Identifies POC and Value Area.

use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};
use crate::error::StudyError;
use crate::orderflow::profile_core;
use crate::output::{ProfileData, ProfileSide, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use data::SerializableColor;

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
                key: "width_pct".into(),
                label: "Width %".into(),
                description: "Profile width as percentage of chart".into(),
                kind: ParameterKind::Float {
                    min: 0.05,
                    max: 0.5,
                    step: 0.05,
                },
                default: ParameterValue::Float(DEFAULT_WIDTH_PCT),
                tab: ParameterTab::Parameters,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "poc_color".into(),
                label: "POC Color".into(),
                description: "Point of Control highlight color".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_POC_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "val_color".into(),
                label: "Value Area Color".into(),
                description: "Value Area fill color".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_VAL_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "var_color".into(),
                label: "Volume Area Color".into(),
                description: "Volume bars outside Value Area".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_VAR_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
        ];

        let mut config = StudyConfig::new("volume_profile");
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

impl Default for VolumeProfileStudy {
    fn default() -> Self {
        Self::new()
    }
}

// Profile computation delegated to profile_core module.

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

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError> {
        if input.candles.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        let levels = profile_core::build_profile_from_candles(
            input.candles,
            input.tick_size,
            input.tick_size.units(),
        );

        if levels.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        let poc = profile_core::find_poc_index(&levels);
        let value_area = poc.and_then(|poc_idx| {
            profile_core::calculate_value_area(&levels, poc_idx, 0.7)
        });

        self.output = StudyOutput::Profile(ProfileData {
            side: ProfileSide::Left,
            levels,
            poc,
            value_area,
            buy_color: SerializableColor::new(0.18, 0.55, 0.82, 0.6),
            sell_color: SerializableColor::new(0.82, 0.28, 0.28, 0.6),
            poc_color: SerializableColor::new(1.0, 0.84, 0.0, 0.8),
            value_area_color: SerializableColor::new(
                0.5, 0.5, 0.5, 0.15,
            ),
            width_pct: 0.3,
        });
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
    use crate::output::ProfileLevel;
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
    fn test_build_profile() {
        let candles = vec![
            make_candle(1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0),
            make_candle(2000, 101.0, 103.0, 100.0, 102.0, 80.0, 60.0),
        ];

        let tick_size = Price::from_f32(1.0);
        let levels = profile_core::build_profile_from_candles(
            &candles,
            tick_size,
            tick_size.units(),
        );

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
                price_units: Price::from_f64(99.0).units(),
                buy_volume: 10.0,
                sell_volume: 5.0,
            },
            ProfileLevel {
                price: 100.0,
                price_units: Price::from_f64(100.0).units(),
                buy_volume: 50.0,
                sell_volume: 40.0,
            },
            ProfileLevel {
                price: 101.0,
                price_units: Price::from_f64(101.0).units(),
                buy_volume: 20.0,
                sell_volume: 10.0,
            },
        ];

        let poc = profile_core::find_poc_index(&levels);
        assert_eq!(poc, Some(1)); // level 100.0 has highest total volume (90)
    }

    #[test]
    fn test_value_area() {
        let levels = vec![
            ProfileLevel {
                price: 98.0,
                price_units: Price::from_f64(98.0).units(),
                buy_volume: 5.0,
                sell_volume: 5.0,
            },
            ProfileLevel {
                price: 99.0,
                price_units: Price::from_f64(99.0).units(),
                buy_volume: 20.0,
                sell_volume: 10.0,
            },
            ProfileLevel {
                price: 100.0,
                price_units: Price::from_f64(100.0).units(),
                buy_volume: 50.0,
                sell_volume: 40.0,
            },
            ProfileLevel {
                price: 101.0,
                price_units: Price::from_f64(101.0).units(),
                buy_volume: 15.0,
                sell_volume: 15.0,
            },
            ProfileLevel {
                price: 102.0,
                price_units: Price::from_f64(102.0).units(),
                buy_volume: 5.0,
                sell_volume: 5.0,
            },
        ];

        let poc_idx = 2; // price 100.0
        let va = profile_core::calculate_value_area(&levels, poc_idx, 0.7);
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

        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Profile(data) => {
                assert!(!data.levels.is_empty());
                assert!(data.poc.is_some());
            }
            other => assert!(matches!(other, StudyOutput::Profile(_)), "Expected Profile output"),
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

        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }
}
