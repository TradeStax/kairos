use super::*;
use crate::config::ParameterValue;
use crate::output::StudyOutput;
use crate::util::test_helpers::{make_candle_ohlcv, make_input};
use data::Candle;

fn make_candle(time: u64, high: f32, low: f32, close: f32) -> Candle {
    make_candle_ohlcv(time, (high + low) / 2.0, high, low, close, 100.0, 100.0)
}

#[test]
fn test_stochastic_empty() {
    let mut study = StochasticStudy::new();
    let input = make_input(&[]);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_stochastic_insufficient() {
    let mut study = StochasticStudy::new();
    // Default k_period=14, needs at least 14 candles
    let candles: Vec<Candle> = (0..5)
        .map(|i| make_candle(i as u64 * 60000, 105.0, 95.0, 100.0))
        .collect();
    let input = make_input(&candles);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_stochastic_calculation() {
    let mut study = StochasticStudy::new();
    study
        .set_parameter("k_period", ParameterValue::Integer(5))
        .unwrap();
    study
        .set_parameter("d_period", ParameterValue::Integer(3))
        .unwrap();
    study
        .set_parameter("smooth", ParameterValue::Integer(1))
        .unwrap();

    // Ascending prices
    let candles: Vec<Candle> = (0..10)
        .map(|i| {
            let base = 100.0 + i as f32 * 5.0;
            make_candle((i + 1) as u64 * 60000, base + 3.0, base - 3.0, base)
        })
        .collect();
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
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].label, "%K");
    assert_eq!(lines[1].label, "%D");
    // In an uptrend, %K should be high (near 100)
    for pt in &lines[0].points {
        assert!(pt.1 > 50.0, "Expected %K > 50 in uptrend, got {}", pt.1);
    }
}

#[test]
fn test_stochastic_range_bound() {
    let mut study = StochasticStudy::new();
    study
        .set_parameter("k_period", ParameterValue::Integer(5))
        .unwrap();
    study
        .set_parameter("d_period", ParameterValue::Integer(1))
        .unwrap();
    study
        .set_parameter("smooth", ParameterValue::Integer(1))
        .unwrap();

    // All candles identical - flat market
    let candles: Vec<Candle> = (0..10)
        .map(|i| make_candle(i as u64 * 60000, 105.0, 95.0, 100.0))
        .collect();
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
    assert_eq!(lines.len(), 2);
    // With identical highs and lows, raw %K should be 50
    // (close - low) / (high - low) = (100-95)/(105-95) = 0.5 * 100 = 50
    for pt in &lines[0].points {
        assert!(
            (pt.1 - 50.0).abs() < 0.1,
            "Expected %K ~ 50.0, got {}",
            pt.1
        );
    }
}

#[test]
fn test_stochastic_k_d_same_length() {
    let mut study = StochasticStudy::new();
    study
        .set_parameter("k_period", ParameterValue::Integer(5))
        .unwrap();
    study
        .set_parameter("d_period", ParameterValue::Integer(3))
        .unwrap();
    study
        .set_parameter("smooth", ParameterValue::Integer(3))
        .unwrap();

    let candles: Vec<Candle> = (0..20)
        .map(|i| {
            let base = 100.0 + (i as f32 * 2.0).sin() * 10.0;
            make_candle(i as u64 * 60000, base + 3.0, base - 3.0, base)
        })
        .collect();
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
    assert_eq!(lines.len(), 2);
    // %K and %D should have the same number of points
    assert_eq!(lines[0].points.len(), lines[1].points.len());
    // And the x-values should match
    for (k, d) in lines[0].points.iter().zip(lines[1].points.iter()) {
        assert_eq!(k.0, d.0);
    }
}

/// H9: Edge case — smooth=1 means no smoothing, %K should equal raw %K
#[test]
fn test_stochastic_smooth_1() {
    let mut study = StochasticStudy::new();
    study
        .set_parameter("k_period", ParameterValue::Integer(5))
        .unwrap();
    study
        .set_parameter("d_period", ParameterValue::Integer(3))
        .unwrap();
    study
        .set_parameter("smooth", ParameterValue::Integer(1))
        .unwrap();

    let candles: Vec<Candle> = (0..15)
        .map(|i| {
            let base = 100.0 + i as f32 * 3.0;
            make_candle(i as u64 * 60000, base + 5.0, base - 5.0, base)
        })
        .collect();
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let output = study.output();
    let StudyOutput::Lines(lines) = output else {
        panic!("expected Lines output");
    };
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].points.len(), lines[1].points.len());
    // All values should be in [0, 100]
    for pt in &lines[0].points {
        assert!(pt.1 >= -0.1 && pt.1 <= 100.1, "%K out of range: {}", pt.1);
    }
    for pt in &lines[1].points {
        assert!(pt.1 >= -0.1 && pt.1 <= 100.1, "%D out of range: {}", pt.1);
    }
}

/// H9: Edge case — d_period=1 means %D = %K (no smoothing of %K)
#[test]
fn test_stochastic_d_period_1() {
    let mut study = StochasticStudy::new();
    study
        .set_parameter("k_period", ParameterValue::Integer(5))
        .unwrap();
    study
        .set_parameter("d_period", ParameterValue::Integer(1))
        .unwrap();
    study
        .set_parameter("smooth", ParameterValue::Integer(3))
        .unwrap();

    let candles: Vec<Candle> = (0..15)
        .map(|i| {
            let base = 100.0 + i as f32 * 3.0;
            make_candle(i as u64 * 60000, base + 5.0, base - 5.0, base)
        })
        .collect();
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let output = study.output();
    let StudyOutput::Lines(lines) = output else {
        panic!("expected Lines output");
    };
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].points.len(), lines[1].points.len());
    // With d_period=1, %D = SMA(1) of %K = %K itself
    for (k, d) in lines[0].points.iter().zip(lines[1].points.iter()) {
        assert!(
            (k.1 - d.1).abs() < 0.01,
            "%K ({}) != %D ({}) when d_period=1",
            k.1,
            d.1
        );
    }
}
