//! Feed Merger Service
//!
//! Merges trade data from multiple feeds, deduplicates overlapping trades,
//! and detects gaps in coverage.

use crate::domain::chart::{DataGap, DataGapKind, DataSegment, MergeResult};
use crate::domain::types::Timestamp;
use crate::feed::FeedId;

/// Deduplication tolerance: trades within this many milliseconds at the same
/// price are considered duplicates across feeds.
const DEDUP_TOLERANCE_MS: u64 = 1;

/// Minimum gap duration (ms) to report. Gaps shorter than this are ignored.
const MIN_GAP_DURATION_MS: u64 = 5_000;

/// Merge data segments from multiple feeds into a single sorted,
/// deduplicated trade list with gap detection.
///
/// Strategy:
/// 1. Sort segments by feed priority (lower priority number = higher priority)
/// 2. Concatenate all trades and sort by time
/// 3. Deduplicate trades within `DEDUP_TOLERANCE_MS` at the same price
/// 4. Detect gaps in the expected time range
pub fn merge_segments(
    mut segments: Vec<DataSegment>,
    expected_start: Timestamp,
    expected_end: Timestamp,
) -> MergeResult {
    if segments.is_empty() {
        return MergeResult {
            trades: Vec::new(),
            gaps: vec![DataGap::new(
                expected_start,
                expected_end,
                DataGapKind::NoData,
            )],
            feed_ids: Vec::new(),
        };
    }

    // Collect unique feed IDs
    let mut feed_ids: Vec<FeedId> = segments.iter().map(|s| s.feed_id).collect();
    feed_ids.sort();
    feed_ids.dedup();

    // Concatenate all trades from all segments
    // Caller should provide segments ordered by feed priority (first = highest)
    let total_trades: usize = segments.iter().map(|s| s.trades.len()).sum();
    let mut all_trades = Vec::with_capacity(total_trades);

    for segment in segments.drain(..) {
        all_trades.extend(segment.trades);
    }

    // Sort by time, then by price for stable dedup
    all_trades.sort_by(|a, b| {
        a.time
            .0
            .cmp(&b.time.0)
            .then_with(|| a.price.units().cmp(&b.price.units()))
    });

    // Deduplicate: remove trades that are within tolerance at the same price
    let trades = dedup_trades(all_trades);

    // Detect gaps
    let gaps = detect_gaps(&trades, expected_start, expected_end);

    MergeResult {
        trades,
        gaps,
        feed_ids,
    }
}

/// Remove duplicate trades within `DEDUP_TOLERANCE_MS` at the same price.
/// Keeps the first occurrence (highest priority feed should be listed first).
fn dedup_trades(trades: Vec<crate::domain::Trade>) -> Vec<crate::domain::Trade> {
    if trades.len() <= 1 {
        return trades;
    }

    let mut result = Vec::with_capacity(trades.len());
    result.push(trades[0]);

    for trade in trades.into_iter().skip(1) {
        let last = result.last().unwrap();
        let time_diff = trade.time.0.abs_diff(last.time.0);
        let same_price = trade.price.units() == last.price.units();

        if time_diff <= DEDUP_TOLERANCE_MS && same_price {
            // Duplicate within tolerance — skip
            continue;
        }

        result.push(trade);
    }

    result
}

/// Detect gaps in the trade timeline.
///
/// A gap is any period longer than `MIN_GAP_DURATION_MS` between consecutive
/// trades where no data exists.
fn detect_gaps(
    trades: &[crate::domain::Trade],
    expected_start: Timestamp,
    expected_end: Timestamp,
) -> Vec<DataGap> {
    let mut gaps = Vec::new();

    if trades.is_empty() {
        if expected_end.0 > expected_start.0 + MIN_GAP_DURATION_MS {
            gaps.push(DataGap::new(
                expected_start,
                expected_end,
                DataGapKind::NoData,
            ));
        }
        return gaps;
    }

    // Gap from expected start to first trade
    let first_time = trades[0].time;
    if first_time.0 > expected_start.0 + MIN_GAP_DURATION_MS {
        gaps.push(DataGap::new(
            expected_start,
            first_time,
            DataGapKind::NoData,
        ));
    }

    // Gaps between consecutive trades
    for window in trades.windows(2) {
        let gap_ms = window[1].time.0.saturating_sub(window[0].time.0);
        if gap_ms > MIN_GAP_DURATION_MS {
            gaps.push(DataGap::new(
                window[0].time,
                window[1].time,
                DataGapKind::NoData,
            ));
        }
    }

    // Gap from last trade to expected end
    let last_time = trades[trades.len() - 1].time;
    if expected_end.0 > last_time.0 + MIN_GAP_DURATION_MS {
        gaps.push(DataGap::new(last_time, expected_end, DataGapKind::NoData));
    }

    gaps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{Price, Quantity, Side};

    fn make_trade(time_ms: u64, price: f32) -> crate::domain::Trade {
        crate::domain::Trade::new(
            Timestamp(time_ms),
            Price::from_f32(price),
            Quantity(1.0),
            Side::Buy,
        )
    }

    #[test]
    fn test_merge_empty_segments() {
        let result = merge_segments(vec![], Timestamp(0), Timestamp(100_000));
        assert!(result.trades.is_empty());
        assert_eq!(result.gaps.len(), 1);
        assert_eq!(result.gaps[0].kind, DataGapKind::NoData);
    }

    #[test]
    fn test_merge_single_segment() {
        let trades = vec![
            make_trade(1000, 100.0),
            make_trade(2000, 101.0),
            make_trade(3000, 100.5),
        ];

        let segment = DataSegment {
            feed_id: FeedId::new_v4(),
            start: Timestamp(1000),
            end: Timestamp(3000),
            trades,
        };

        let result = merge_segments(vec![segment], Timestamp(1000), Timestamp(3000));

        assert_eq!(result.trades.len(), 3);
        assert!(result.gaps.is_empty());
    }

    #[test]
    fn test_dedup_overlapping_trades() {
        let feed_a = FeedId::new_v4();
        let feed_b = FeedId::new_v4();

        let seg_a = DataSegment {
            feed_id: feed_a,
            start: Timestamp(1000),
            end: Timestamp(3000),
            trades: vec![
                make_trade(1000, 100.0),
                make_trade(2000, 101.0),
                make_trade(3000, 102.0),
            ],
        };

        let seg_b = DataSegment {
            feed_id: feed_b,
            start: Timestamp(1000),
            end: Timestamp(3000),
            trades: vec![
                make_trade(1000, 100.0), // duplicate
                make_trade(2001, 101.0), // within 1ms tolerance, same price = dup
                make_trade(3000, 103.0), // different price = kept
            ],
        };

        let result = merge_segments(vec![seg_a, seg_b], Timestamp(1000), Timestamp(3000));

        // 1000@100, 2000@101, 3000@102, 3000@103 = 4 unique
        assert_eq!(result.trades.len(), 4);
        assert_eq!(result.feed_ids.len(), 2);
    }

    #[test]
    fn test_gap_detection() {
        let segment = DataSegment {
            feed_id: FeedId::new_v4(),
            start: Timestamp(0),
            end: Timestamp(100_000),
            trades: vec![
                make_trade(10_000, 100.0),
                // 50s gap here (> MIN_GAP_DURATION_MS)
                make_trade(60_000, 101.0),
                make_trade(61_000, 102.0),
            ],
        };

        let result = merge_segments(vec![segment], Timestamp(0), Timestamp(100_000));

        assert_eq!(result.trades.len(), 3);
        // Gaps: 0->10000 (leading), 10000->60000 (middle), 61000->100000 (trailing)
        assert_eq!(result.gaps.len(), 3);
    }

    #[test]
    fn test_no_gaps_when_continuous() {
        let trades: Vec<_> = (0..20)
            .map(|i| make_trade(i * 200, 100.0 + i as f32 * 0.25))
            .collect();

        let end_time = 19 * 200;
        let segment = DataSegment {
            feed_id: FeedId::new_v4(),
            start: Timestamp(0),
            end: Timestamp(end_time),
            trades,
        };

        let result = merge_segments(vec![segment], Timestamp(0), Timestamp(end_time));

        assert_eq!(result.trades.len(), 20);
        assert!(result.gaps.is_empty());
    }
}
