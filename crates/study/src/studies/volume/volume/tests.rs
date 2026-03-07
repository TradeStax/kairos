use super::*;
use crate::util::test_helpers::make_input;
use data::{Candle, Price, Timestamp, Volume};

fn make_candle(time: u64, open: f32, close: f32, vol: f64) -> Candle {
    let high = open.max(close) + 1.0;
    let low = open.min(close) - 1.0;
    Candle::new(
        Timestamp::from_millis(time),
        Price::from_f32(open),
        Price::from_f32(high),
        Price::from_f32(low),
        Price::from_f32(close),
        Volume(vol * 0.6),
        Volume(vol * 0.4),
    )
    .expect("test: valid candle")
}

#[test]
fn test_volume_basic() {
    let mut study = VolumeStudy::new();
    let candles = vec![
        make_candle(1000, 100.0, 102.0, 500.0), // bullish
        make_candle(2000, 102.0, 99.0, 300.0),  // bearish
        make_candle(3000, 99.0, 101.0, 400.0),  // bullish
    ];

    let input = make_input(&candles);

    study.compute(&input).unwrap();

    match &study.output {
        StudyOutput::Bars(series) => {
            assert_eq!(series.len(), 1);
            assert_eq!(series[0].points.len(), 3);
            // Check volume values
            assert!((series[0].points[0].value - 500.0).abs() < 1.0);
            assert!((series[0].points[1].value - 300.0).abs() < 1.0);
            // Bullish bar should be green-ish
            assert!(series[0].points[0].color.g > series[0].points[0].color.r);
            // Bearish bar should be red-ish
            assert!(series[0].points[1].color.r > series[0].points[1].color.g);
        }
        other => assert!(
            matches!(other, StudyOutput::Bars(_)),
            "Expected Bars output"
        ),
    }
}

#[test]
fn test_volume_empty() {
    let mut study = VolumeStudy::new();
    let candles: Vec<Candle> = vec![];

    let input = make_input(&candles);

    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_volume_reset() {
    let mut study = VolumeStudy::new();
    let candles = vec![make_candle(1000, 100.0, 102.0, 500.0)];

    let input = make_input(&candles);

    study.compute(&input).unwrap();
    assert!(!matches!(study.output(), StudyOutput::Empty));

    study.reset();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn single_candle_produces_one_bar() {
    let mut study = VolumeStudy::new();
    let candles = vec![make_candle(1000, 100.0, 105.0, 200.0)];

    let input = make_input(&candles);

    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Bars(series) => {
            assert_eq!(series.len(), 1);
            assert_eq!(series[0].points.len(), 1);
            assert!((series[0].points[0].value - 200.0).abs() < 1.0);
        }
        _ => panic!("Expected Bars output"),
    }
}

#[test]
fn zero_volume_candle_produces_zero_bar() {
    let mut study = VolumeStudy::new();
    let candles = vec![
        Candle::new(
            Timestamp::from_millis(1000),
            Price::from_f32(100.0),
            Price::from_f32(101.0),
            Price::from_f32(99.0),
            Price::from_f32(100.5),
            Volume(0.0),
            Volume(0.0),
        )
        .expect("valid candle"),
    ];

    let input = make_input(&candles);

    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Bars(series) => {
            assert_eq!(series[0].points.len(), 1);
            assert!(series[0].points[0].value.abs() < 0.01);
        }
        _ => panic!("Expected Bars output"),
    }
}

#[test]
fn flat_candle_uses_up_color() {
    // close == open => bullish coloring (close >= open)
    let mut study = VolumeStudy::new();
    let candles = vec![
        Candle::new(
            Timestamp::from_millis(1000),
            Price::from_f32(100.0),
            Price::from_f32(101.0),
            Price::from_f32(99.0),
            Price::from_f32(100.0),
            Volume(50.0),
            Volume(50.0),
        )
        .expect("valid candle"),
    ];

    let input = make_input(&candles);

    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Bars(series) => {
            // close >= open, so should use up_color (green-ish)
            assert!(series[0].points[0].color.g > series[0].points[0].color.r);
        }
        _ => panic!("Expected Bars output"),
    }
}

#[test]
fn volume_is_sum_of_buy_and_sell() {
    let mut study = VolumeStudy::new();
    let candles = vec![
        Candle::new(
            Timestamp::from_millis(1000),
            Price::from_f32(100.0),
            Price::from_f32(103.0),
            Price::from_f32(98.0),
            Price::from_f32(102.0),
            Volume(300.0),
            Volume(200.0),
        )
        .expect("valid candle"),
    ];

    let input = make_input(&candles);

    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Bars(series) => {
            // Total volume = buy + sell = 300 + 200 = 500
            assert!((series[0].points[0].value - 500.0).abs() < 1.0);
        }
        _ => panic!("Expected Bars output"),
    }
}
