//! Timeframe aggregation for OHLCV bars

use crate::{Kline, Timeframe};

/// Aggregate 1-minute bars to target timeframe
pub(crate) fn aggregate_to_timeframe(klines_1m: Vec<Kline>, target: Timeframe) -> Vec<Kline> {
    match target {
        Timeframe::M1 => klines_1m, // No aggregation needed
        Timeframe::M3 => aggregate_minutes(klines_1m, 3),
        Timeframe::M5 => aggregate_minutes(klines_1m, 5),
        Timeframe::M15 => aggregate_minutes(klines_1m, 15),
        Timeframe::M30 => aggregate_minutes(klines_1m, 30),
        Timeframe::H1 => aggregate_minutes(klines_1m, 60),
        Timeframe::H4 => aggregate_minutes(klines_1m, 240),
        Timeframe::D1 => aggregate_minutes(klines_1m, 1440),
        _ => {
            log::warn!(
                "Unsupported timeframe for aggregation: {:?}, returning 1M bars",
                target
            );
            klines_1m
        }
    }
}

/// Aggregate 1-minute bars into N-minute bars
fn aggregate_minutes(bars_1m: Vec<Kline>, minutes: u32) -> Vec<Kline> {
    if bars_1m.is_empty() {
        return Vec::new();
    }

    let interval_ms = (minutes as u64) * 60 * 1000;
    let mut aggregated = Vec::new();
    let mut current_group = Vec::new();

    let mut group_start_time = (bars_1m[0].time / interval_ms) * interval_ms;

    for bar in bars_1m {
        let bar_group_time = (bar.time / interval_ms) * interval_ms;

        if bar_group_time != group_start_time {
            // New group - aggregate previous
            if !current_group.is_empty() {
                aggregated.push(aggregate_group(&current_group, group_start_time));
                current_group.clear();
            }
            group_start_time = bar_group_time;
        }

        current_group.push(bar);
    }

    // Aggregate final group
    if !current_group.is_empty() {
        aggregated.push(aggregate_group(&current_group, group_start_time));
    }

    aggregated
}

/// Aggregate a group of bars into one bar
fn aggregate_group(bars: &[Kline], time: u64) -> Kline {
    debug_assert!(!bars.is_empty(), "aggregate_group called with empty bars");
    let open = bars.first().expect("aggregate_group: empty bars").open;
    let close = bars.last().expect("aggregate_group: empty bars").close;
    let high = bars
        .iter()
        .map(|b| b.high)
        .fold(f32::NEG_INFINITY, f32::max);
    let low = bars.iter().map(|b| b.low).fold(f32::INFINITY, f32::min);
    let volume = bars.iter().map(|b| b.volume).sum();
    let buy_volume = bars.iter().map(|b| b.buy_volume).sum();
    let sell_volume = bars.iter().map(|b| b.sell_volume).sum();

    Kline {
        time,
        open,
        high,
        low,
        close,
        volume,
        buy_volume,
        sell_volume,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_timeframe() {
        // Test aggregation from 1M to 5M
        let klines_1m = vec![
            // 5 consecutive 1-minute bars
            create_test_kline(0, 100.0, 102.0, 99.0, 101.0, 10.0),
            create_test_kline(60000, 101.0, 103.0, 100.0, 102.0, 15.0),
            create_test_kline(120000, 102.0, 104.0, 101.0, 103.0, 20.0),
            create_test_kline(180000, 103.0, 105.0, 102.0, 104.0, 25.0),
            create_test_kline(240000, 104.0, 106.0, 103.0, 105.0, 30.0),
        ];

        let result = aggregate_minutes(klines_1m, 5);

        assert_eq!(result.len(), 1);
        assert!((result[0].open - 100.0).abs() < 0.01); // First's open
        assert!((result[0].high - 106.0).abs() < 0.01); // Max high
        assert!((result[0].low - 99.0).abs() < 0.01); // Min low
        assert!((result[0].close - 105.0).abs() < 0.01); // Last's close
        assert!((result[0].volume - 100.0).abs() < 0.01); // Sum volume
    }

    fn create_test_kline(
        time: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        volume: f32,
    ) -> Kline {
        Kline {
            time,
            open,
            high,
            low,
            close,
            volume,
            buy_volume: volume * 0.5,
            sell_volume: volume * 0.5,
        }
    }
}
