use super::*;
use crate::config::ParameterValue;
use crate::output::{LineSeries, StudyOutput};
use crate::util::test_helpers::{make_candle, make_input};
use data::Candle;

/// Extract the RSI line series from the Composite output.
fn extract_lines(output: &StudyOutput) -> &[LineSeries] {
    let StudyOutput::Composite(parts) = output else {
        panic!("expected Composite output, got {:?}", output);
    };
    let StudyOutput::Lines(lines) = &parts[0] else {
        panic!("expected Lines as first Composite element");
    };
    lines
}

#[test]
fn test_empty_candles() {
    let mut study = RsiStudy::new();
    let input = make_input(&[]);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_insufficient_candles() {
    let mut study = RsiStudy::new();
    let candles: Vec<Candle> = (0..10).map(|i| make_candle(i * 60000, 100.0)).collect();
    let input = make_input(&candles);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_rsi_all_gains() {
    let mut study = RsiStudy::new();
    study
        .set_parameter("period", ParameterValue::Integer(3))
        .unwrap();

    // Strictly increasing prices: RSI should be 100
    let candles = vec![
        make_candle(1000, 10.0),
        make_candle(2000, 20.0),
        make_candle(3000, 30.0),
        make_candle(4000, 40.0),
    ];
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let lines = extract_lines(study.output());
    assert_eq!(lines[0].points.len(), 1);
    assert!((lines[0].points[0].1 - 100.0).abs() < 0.01);
}

#[test]
fn test_rsi_all_losses() {
    let mut study = RsiStudy::new();
    study
        .set_parameter("period", ParameterValue::Integer(3))
        .unwrap();

    // Strictly decreasing prices: RSI should be 0
    let candles = vec![
        make_candle(1000, 40.0),
        make_candle(2000, 30.0),
        make_candle(3000, 20.0),
        make_candle(4000, 10.0),
    ];
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let lines = extract_lines(study.output());
    assert_eq!(lines[0].points.len(), 1);
    assert!(lines[0].points[0].1.abs() < 0.01);
}

#[test]
fn test_rsi_range() {
    let mut study = RsiStudy::new();
    study
        .set_parameter("period", ParameterValue::Integer(3))
        .unwrap();

    let candles = vec![
        make_candle(1000, 44.0),
        make_candle(2000, 44.25),
        make_candle(3000, 44.5),
        make_candle(4000, 43.75),
        make_candle(5000, 44.5),
        make_candle(6000, 44.25),
    ];
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let lines = extract_lines(study.output());
    for point in &lines[0].points {
        assert!(point.1 >= 0.0 && point.1 <= 100.0);
    }
}

#[test]
fn test_rsi_levels_in_output() {
    let mut study = RsiStudy::new();
    study
        .set_parameter("period", ParameterValue::Integer(3))
        .unwrap();

    let candles = vec![
        make_candle(1000, 10.0),
        make_candle(2000, 20.0),
        make_candle(3000, 30.0),
        make_candle(4000, 40.0),
    ];
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let StudyOutput::Composite(parts) = study.output() else {
        panic!("expected Composite output");
    };
    assert_eq!(parts.len(), 2);
    assert!(matches!(&parts[0], StudyOutput::Lines(_)));
    let StudyOutput::Levels(levels) = &parts[1] else {
        panic!("expected Levels as second element");
    };
    assert_eq!(levels.len(), 2);
    assert!((levels[0].price - 70.0).abs() < 0.001);
    assert!((levels[1].price - 30.0).abs() < 0.001);
}

#[test]
fn test_set_parameter_valid() {
    let mut study = RsiStudy::new();
    assert!(
        study
            .set_parameter("period", ParameterValue::Integer(21))
            .is_ok()
    );
    assert!(
        study
            .set_parameter("overbought", ParameterValue::Float(80.0))
            .is_ok()
    );
}

#[test]
fn test_set_parameter_invalid() {
    let mut study = RsiStudy::new();
    assert!(
        study
            .set_parameter("period", ParameterValue::Integer(1))
            .is_err()
    );
    assert!(
        study
            .set_parameter("period", ParameterValue::Integer(101))
            .is_err()
    );
    assert!(
        study
            .set_parameter("overbought", ParameterValue::Float(40.0))
            .is_err()
    );
    assert!(
        study
            .set_parameter("unknown", ParameterValue::Integer(1))
            .is_err()
    );
}
