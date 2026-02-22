use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::util::candle_key;
use data::SerializableColor;

const DEFAULT_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 0.6,
    b: 0.0,
    a: 1.0,
};

fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "period".into(),
            label: "Period".into(),
            description: "Number of candles for ATR calculation".into(),
            kind: ParameterKind::Integer { min: 1, max: 100 },
            default: ParameterValue::Integer(14),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "color".into(),
            label: "Color".into(),
            description: "ATR line color".into(),
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
    ]
}

pub struct AtrStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl AtrStudy {
    pub fn new() -> Self {
        let params = make_params();
        let mut config = StudyConfig::new("atr");
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

impl Default for AtrStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for AtrStudy {
    fn id(&self) -> &str {
        "atr"
    }

    fn name(&self) -> &str {
        "Average True Range"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::Volatility
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
        let period = self.config.get_int("period", 14) as usize;
        let color = self.config.get_color("color", DEFAULT_COLOR);
        let width = self.config.get_float("width", 1.5) as f32;

        let candles = input.candles;
        if candles.len() < 2 {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Calculate True Range for each candle (starting from index 1)
        let mut tr_values = Vec::with_capacity(candles.len() - 1);
        for i in 1..candles.len() {
            let high = candles[i].high.to_f32() as f64;
            let low = candles[i].low.to_f32() as f64;
            let prev_close = candles[i - 1].close.to_f32() as f64;

            let tr = (high - low)
                .max((high - prev_close).abs())
                .max((low - prev_close).abs());
            tr_values.push(tr);
        }

        if tr_values.len() < period {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        let mut points = Vec::with_capacity(tr_values.len() - period + 1);

        // Initial ATR: simple average of first `period` TR values
        let mut atr: f64 = tr_values[..period].iter().sum::<f64>() / period as f64;
        // The first ATR corresponds to candle at index period (since TR
        // starts at index 1)
        let candle_idx = period;
        points.push((
            candle_key(&candles[candle_idx], candle_idx, candles.len(), &input.basis),
            atr as f32,
        ));

        // Wilder's smoothing: ATR = (prev_ATR * (period-1) + TR) / period
        for (i, tr) in tr_values.iter().enumerate().skip(period) {
            atr = (atr * (period as f64 - 1.0) + tr) / period as f64;
            let candle_idx = i + 1; // offset by 1 since TR starts at index 1
            points.push((
                candle_key(&candles[candle_idx], candle_idx, candles.len(), &input.basis),
                atr as f32,
            ));
        }

        self.output = StudyOutput::Lines(vec![LineSeries {
            label: format!("ATR({})", period),
            color,
            width,
            style: crate::config::LineStyleValue::Solid,
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

    fn make_candle(time: u64, open: f32, high: f32, low: f32, close: f32) -> Candle {
        Candle::new(
            Timestamp(time),
            Price::from_f32(open),
            Price::from_f32(high),
            Price::from_f32(low),
            Price::from_f32(close),
            Volume(100.0),
            Volume(100.0),
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
    fn test_atr_empty() {
        let mut study = AtrStudy::new();
        let input = make_input(&[]);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_atr_insufficient() {
        let mut study = AtrStudy::new();
        // Default period 14, need at least 15 candles (period + 1 for prev_close)
        let candles: Vec<Candle> = (0..5)
            .map(|i| make_candle(i * 60000, 100.0, 102.0, 98.0, 101.0))
            .collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_atr_calculation() {
        let mut study = AtrStudy::new();
        study
            .set_parameter("period", ParameterValue::Integer(3))
            .unwrap();

        // Create candles with known true ranges
        let candles = vec![
            make_candle(1000, 100.0, 105.0, 95.0, 102.0),  // base
            make_candle(2000, 102.0, 108.0, 98.0, 104.0),  // TR = max(10, |108-102|, |98-102|) = 10
            make_candle(3000, 104.0, 110.0, 100.0, 106.0), // TR = max(10, |110-104|, |100-104|) = 10
            make_candle(4000, 106.0, 115.0, 103.0, 112.0), // TR = max(12, |115-106|, |103-106|) = 12
            make_candle(5000, 112.0, 118.0, 108.0, 116.0), // TR = max(10, |118-112|, |108-112|) = 10
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        assert_eq!(lines.len(), 1);
        let pts = &lines[0].points;
        assert_eq!(pts.len(), 2);

        // Initial ATR(3) = avg of first 3 TRs = (10+10+12)/3 = 10.667
        assert!((pts[0].1 - 10.667).abs() < 0.01);

        // Wilder: ATR = (10.667 * 2 + 10) / 3 = 31.333/3 = 10.444
        assert!((pts[1].1 - 10.444).abs() < 0.01);
    }

    #[test]
    fn test_atr_constant_range() {
        let mut study = AtrStudy::new();
        study
            .set_parameter("period", ParameterValue::Integer(2))
            .unwrap();

        // All candles have the same range, no gaps
        let candles = vec![
            make_candle(1000, 100.0, 105.0, 95.0, 100.0),
            make_candle(2000, 100.0, 105.0, 95.0, 100.0), // TR = 10
            make_candle(3000, 100.0, 105.0, 95.0, 100.0), // TR = 10
            make_candle(4000, 100.0, 105.0, 95.0, 100.0), // TR = 10
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        let pts = &lines[0].points;
        // All ATR values should be 10.0
        for pt in pts {
            assert!((pt.1 - 10.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_set_parameter_valid() {
        let mut study = AtrStudy::new();
        assert!(
            study
                .set_parameter("period", ParameterValue::Integer(20))
                .is_ok()
        );
    }

    #[test]
    fn test_set_parameter_invalid_range() {
        let mut study = AtrStudy::new();
        assert!(
            study
                .set_parameter("period", ParameterValue::Integer(0))
                .is_err()
        );
        assert!(
            study
                .set_parameter("period", ParameterValue::Integer(101))
                .is_err()
        );
    }

    #[test]
    fn test_set_parameter_unknown() {
        let mut study = AtrStudy::new();
        assert!(
            study
                .set_parameter("unknown", ParameterValue::Integer(5))
                .is_err()
        );
    }
}
