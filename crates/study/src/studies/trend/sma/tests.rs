use super::*;
use crate::config::ParameterValue;
use crate::util::test_helpers::{make_candle, make_input};
use data::Candle;

#[test]
fn test_empty_candles() {
    let mut study = SmaStudy::new();
    let input = make_input(&[]);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_insufficient_candles() {
    let mut study = SmaStudy::new();
    // Default period is 20, so 5 candles is insufficient
    let candles: Vec<Candle> = (0..5).map(|i| make_candle(i * 60000, 100.0)).collect();
    let input = make_input(&candles);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_sma_calculation() {
    let mut study = SmaStudy::new();
    study
        .set_parameter("period", ParameterValue::Integer(3))
        .unwrap();

    let candles = vec![
        make_candle(1000, 10.0),
        make_candle(2000, 20.0),
        make_candle(3000, 30.0),
        make_candle(4000, 40.0),
        make_candle(5000, 50.0),
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
    let points = &lines[0].points;
    assert_eq!(points.len(), 3);
    // SMA(3) of [10, 20, 30] = 20.0
    assert!((points[0].1 - 20.0).abs() < 0.01);
    // SMA(3) of [20, 30, 40] = 30.0
    assert!((points[1].1 - 30.0).abs() < 0.01);
    // SMA(3) of [30, 40, 50] = 40.0
    assert!((points[2].1 - 40.0).abs() < 0.01);
}

#[test]
fn test_set_parameter_valid() {
    let mut study = SmaStudy::new();
    assert!(
        study
            .set_parameter("period", ParameterValue::Integer(50))
            .is_ok()
    );
}

#[test]
fn test_set_parameter_invalid_range() {
    let mut study = SmaStudy::new();
    assert!(
        study
            .set_parameter("period", ParameterValue::Integer(0))
            .is_err()
    );
    assert!(
        study
            .set_parameter("period", ParameterValue::Integer(501))
            .is_err()
    );
}

#[test]
fn test_set_parameter_wrong_type() {
    let mut study = SmaStudy::new();
    assert!(
        study
            .set_parameter("period", ParameterValue::Float(5.0))
            .is_err()
    );
}

#[test]
fn test_set_parameter_unknown() {
    let mut study = SmaStudy::new();
    assert!(
        study
            .set_parameter("unknown", ParameterValue::Integer(5))
            .is_err()
    );
}
