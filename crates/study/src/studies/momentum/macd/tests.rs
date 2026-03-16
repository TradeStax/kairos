use super::*;
use crate::config::ParameterValue;
use crate::output::StudyOutput;
use crate::util::test_helpers::{make_candle, make_input};
use data::Candle;

#[test]
fn test_empty_candles() {
    let mut study = MacdStudy::new();
    let input = make_input(&[]);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_insufficient_candles() {
    let mut study = MacdStudy::new();
    let candles: Vec<Candle> = (0..20).map(|i| make_candle(i * 60000, 100.0)).collect();
    let input = make_input(&candles);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_macd_constant_price() {
    let mut study = MacdStudy::new();
    study
        .set_parameter("fast_period", ParameterValue::Integer(3))
        .unwrap();
    study
        .set_parameter("slow_period", ParameterValue::Integer(5))
        .unwrap();
    study
        .set_parameter("signal_period", ParameterValue::Integer(2))
        .unwrap();

    // With constant prices, MACD should be ~0
    let candles: Vec<Candle> = (0..20).map(|i| make_candle(i * 60000, 100.0)).collect();
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Composite(_)),
        "expected Composite output"
    );
    let StudyOutput::Composite(outputs) = output else {
        unreachable!()
    };
    assert!(outputs.len() >= 2);
    let StudyOutput::Lines(lines) = &outputs[0] else {
        panic!("expected Lines first")
    };
    assert_eq!(lines.len(), 2);
    // MACD line should be near 0 for constant prices
    for point in &lines[0].points {
        assert!(point.1.abs() < 0.01, "MACD should be ~0, got {}", point.1);
    }
}

#[test]
fn test_macd_trending_price() {
    let mut study = MacdStudy::new();
    study
        .set_parameter("fast_period", ParameterValue::Integer(3))
        .unwrap();
    study
        .set_parameter("slow_period", ParameterValue::Integer(5))
        .unwrap();
    study
        .set_parameter("signal_period", ParameterValue::Integer(2))
        .unwrap();

    // Rising prices: fast EMA > slow EMA, so MACD > 0
    let candles: Vec<Candle> = (0..20)
        .map(|i| make_candle(i * 60000, 100.0 + i as f32 * 10.0))
        .collect();
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Composite(_)),
        "expected Composite output"
    );
    let StudyOutput::Composite(outputs) = output else {
        unreachable!()
    };
    assert!(outputs.len() >= 2);
    let StudyOutput::Lines(lines) = &outputs[0] else {
        panic!("expected Lines first")
    };
    assert_eq!(lines.len(), 2);
    // In a strong uptrend, MACD should be positive
    for point in &lines[0].points {
        assert!(point.1 > 0.0, "MACD should be positive in uptrend");
    }
}

#[test]
fn test_set_parameter_valid() {
    let mut study = MacdStudy::new();
    assert!(
        study
            .set_parameter("fast_period", ParameterValue::Integer(8))
            .is_ok()
    );
    assert!(
        study
            .set_parameter("slow_period", ParameterValue::Integer(21))
            .is_ok()
    );
    assert!(
        study
            .set_parameter("signal_period", ParameterValue::Integer(5))
            .is_ok()
    );
}

#[test]
fn test_set_parameter_invalid() {
    let mut study = MacdStudy::new();
    assert!(
        study
            .set_parameter("fast_period", ParameterValue::Integer(1))
            .is_err()
    );
    assert!(
        study
            .set_parameter("slow_period", ParameterValue::Integer(201))
            .is_err()
    );
    assert!(
        study
            .set_parameter("unknown", ParameterValue::Integer(5))
            .is_err()
    );
}

#[test]
fn test_macd_alignment_all_three_series() {
    // H10: Verify MACD, Signal, and Histogram all align correctly
    let mut study = MacdStudy::new();
    study
        .set_parameter("fast_period", ParameterValue::Integer(3))
        .unwrap();
    study
        .set_parameter("slow_period", ParameterValue::Integer(5))
        .unwrap();
    study
        .set_parameter("signal_period", ParameterValue::Integer(2))
        .unwrap();

    let candles: Vec<Candle> = (0..20)
        .map(|i| make_candle(i * 60000, 100.0 + i as f32 * 5.0))
        .collect();
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let output = study.output();
    let StudyOutput::Composite(outputs) = output else {
        panic!("expected Composite output");
    };
    let StudyOutput::Lines(lines) = &outputs[0] else {
        panic!("expected Lines first");
    };
    let StudyOutput::Histogram(hist) = &outputs[1] else {
        panic!("expected Histogram second");
    };

    let macd_pts = &lines[0].points;
    let signal_pts = &lines[1].points;

    // All three series must have the same length
    assert_eq!(
        macd_pts.len(),
        signal_pts.len(),
        "MACD and Signal must have same length"
    );
    assert_eq!(
        macd_pts.len(),
        hist.len(),
        "MACD and Histogram must have same length"
    );

    // X-values must all match
    for i in 0..macd_pts.len() {
        assert_eq!(
            macd_pts[i].0, signal_pts[i].0,
            "MACD and Signal x-values must match at index {}",
            i
        );
        assert_eq!(
            macd_pts[i].0, hist[i].x,
            "MACD and Histogram x-values must match at index {}",
            i
        );
    }

    // Histogram = MACD - Signal at each point
    for i in 0..macd_pts.len() {
        let expected_hist = macd_pts[i].1 - signal_pts[i].1;
        assert!(
            (hist[i].value - expected_hist).abs() < 0.01,
            "Histogram[{}] = {} but MACD-Signal = {}",
            i,
            hist[i].value,
            expected_hist
        );
    }
}
