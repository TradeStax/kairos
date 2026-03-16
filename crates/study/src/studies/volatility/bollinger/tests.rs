use super::*;
use crate::config::ParameterValue;
use crate::util::test_helpers::{make_candle, make_input};

#[test]
fn test_empty_candles() {
    let mut study = BollingerStudy::new();
    let input = make_input(&[]);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_bollinger_calculation() {
    let mut study = BollingerStudy::new();
    study
        .set_parameter("period", ParameterValue::Integer(3))
        .unwrap();
    study
        .set_parameter("std_dev", ParameterValue::Float(1.0))
        .unwrap();

    // Use constant values so stddev = 0
    let candles = vec![
        make_candle(1000, 100.0),
        make_candle(2000, 100.0),
        make_candle(3000, 100.0),
    ];
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Band { .. }),
        "expected Band output"
    );
    let StudyOutput::Band {
        upper,
        middle,
        lower,
        ..
    } = output
    else {
        unreachable!()
    };
    assert_eq!(upper.points.len(), 1);
    assert_eq!(lower.points.len(), 1);
    let mid = middle.as_ref().unwrap();
    // All same price: mean = 100, stddev = 0
    assert!((mid.points[0].1 - 100.0).abs() < 0.01);
    assert!((upper.points[0].1 - 100.0).abs() < 0.01);
    assert!((lower.points[0].1 - 100.0).abs() < 0.01);
}

#[test]
fn test_bollinger_with_variance() {
    let mut study = BollingerStudy::new();
    study
        .set_parameter("period", ParameterValue::Integer(3))
        .unwrap();
    study
        .set_parameter("std_dev", ParameterValue::Float(2.0))
        .unwrap();

    let candles = vec![
        make_candle(1000, 10.0),
        make_candle(2000, 20.0),
        make_candle(3000, 30.0),
    ];
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Band { .. }),
        "expected Band output"
    );
    let StudyOutput::Band {
        upper,
        middle,
        lower,
        ..
    } = output
    else {
        unreachable!()
    };
    let mid = middle.as_ref().unwrap();
    // mean = 20.0, variance = ((10-20)^2 + (20-20)^2 + (30-20)^2) / 3
    //       = (100 + 0 + 100) / 3 = 66.67, stddev ~ 8.165
    assert!((mid.points[0].1 - 20.0).abs() < 0.1);
    assert!(upper.points[0].1 > 35.0); // 20 + 2*8.165 ~ 36.33
    assert!(lower.points[0].1 < 5.0); // 20 - 2*8.165 ~ 3.67
}

#[test]
fn test_set_parameter_valid() {
    let mut study = BollingerStudy::new();
    assert!(
        study
            .set_parameter("std_dev", ParameterValue::Float(3.0))
            .is_ok()
    );
}

#[test]
fn test_set_parameter_invalid() {
    let mut study = BollingerStudy::new();
    assert!(
        study
            .set_parameter("std_dev", ParameterValue::Float(6.0))
            .is_err()
    );
    assert!(
        study
            .set_parameter("unknown", ParameterValue::Integer(1))
            .is_err()
    );
}
