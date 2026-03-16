use super::*;
use crate::config::ParameterValue;
use crate::util::test_helpers::{make_candle_ohlcv, make_input};
use data::Candle;

fn make_candle(time: u64, open: f32, high: f32, low: f32, close: f32) -> Candle {
    make_candle_ohlcv(time, open, high, low, close, 100.0, 100.0)
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
    assert!(
        matches!(output, StudyOutput::Lines(_)),
        "expected Lines output"
    );
    let StudyOutput::Lines(lines) = output else {
        unreachable!()
    };
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
    assert!(
        matches!(output, StudyOutput::Lines(_)),
        "expected Lines output"
    );
    let StudyOutput::Lines(lines) = output else {
        unreachable!()
    };
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
