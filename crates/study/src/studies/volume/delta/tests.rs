use super::*;
use crate::util::test_helpers::{make_candle_ohlcv, make_input};
use data::Candle;

fn make_candle(time: u64, buy_vol: f64, sell_vol: f64) -> Candle {
    make_candle_ohlcv(time, 100.0, 102.0, 99.0, 101.0, buy_vol, sell_vol)
}

#[test]
fn test_delta_basic() {
    let mut study = DeltaStudy::new();
    let candles = vec![
        make_candle(1000, 300.0, 200.0), // delta = +100
        make_candle(2000, 100.0, 250.0), // delta = -150
        make_candle(3000, 200.0, 200.0), // delta = 0
    ];

    let input = make_input(&candles);

    study.compute(&input).unwrap();

    match &study.output {
        StudyOutput::Bars(series) => {
            assert_eq!(series.len(), 1);
            let pts = &series[0].points;
            assert_eq!(pts.len(), 3);
            assert!((pts[0].value - 100.0).abs() < 1.0);
            assert!((pts[1].value - (-150.0)).abs() < 1.0);
            assert!((pts[2].value).abs() < 1.0);
            // Positive delta should be green-ish
            assert!(pts[0].color.g > pts[0].color.r);
            // Negative delta should be red-ish
            assert!(pts[1].color.r > pts[1].color.g);
        }
        other => assert!(
            matches!(other, StudyOutput::Bars(_)),
            "Expected Bars output"
        ),
    }
}

#[test]
fn test_delta_empty() {
    let mut study = DeltaStudy::new();
    let candles: Vec<Candle> = vec![];
    let input = make_input(&candles);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn single_candle_delta() {
    let mut study = DeltaStudy::new();
    let candles = vec![make_candle(1000, 500.0, 200.0)]; // delta = +300
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Bars(series) => {
            assert_eq!(series[0].points.len(), 1);
            assert!((series[0].points[0].value - 300.0).abs() < 1.0);
        }
        _ => panic!("Expected Bars output"),
    }
}

#[test]
fn zero_volume_candle_produces_zero_delta() {
    let mut study = DeltaStudy::new();
    let candles = vec![make_candle(1000, 0.0, 0.0)]; // delta = 0
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Bars(series) => {
            assert_eq!(series[0].points.len(), 1);
            assert!(series[0].points[0].value.abs() < 0.01);
            // Zero delta >= 0, so should use positive color (green-ish)
            assert!(series[0].points[0].color.g > series[0].points[0].color.r);
        }
        _ => panic!("Expected Bars output"),
    }
}

#[test]
fn all_buy_volume_positive_delta() {
    let mut study = DeltaStudy::new();
    let candles = vec![make_candle(1000, 400.0, 0.0)]; // delta = +400
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Bars(series) => {
            assert!((series[0].points[0].value - 400.0).abs() < 1.0);
            // Positive delta -> green
            assert!(series[0].points[0].color.g > series[0].points[0].color.r);
        }
        _ => panic!("Expected Bars output"),
    }
}

#[test]
fn all_sell_volume_negative_delta() {
    let mut study = DeltaStudy::new();
    let candles = vec![make_candle(1000, 0.0, 350.0)]; // delta = -350
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Bars(series) => {
            assert!((series[0].points[0].value - (-350.0)).abs() < 1.0);
            // Negative delta -> red
            assert!(series[0].points[0].color.r > series[0].points[0].color.g);
        }
        _ => panic!("Expected Bars output"),
    }
}

#[test]
fn delta_reset_clears_output() {
    let mut study = DeltaStudy::new();
    let candles = vec![make_candle(1000, 300.0, 200.0)];
    let input = make_input(&candles);
    study.compute(&input).unwrap();
    assert!(!matches!(study.output(), StudyOutput::Empty));

    study.reset();
    assert!(matches!(study.output(), StudyOutput::Empty));
}
