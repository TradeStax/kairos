//! Feed merger — combines trade data from multiple feeds with dedup and gap detection.
//!
//! Intended for app-layer use: collect [`DataSegment`]s from multiple adapters,
//! then merge into a single sorted, deduplicated trade list. The
//! [`DataEngine`](super::DataEngine) itself stays single-adapter and does not
//! call this internally.

use crate::domain::chart::{DataGap, DataGapKind, DataSegment, MergeResult};
use crate::domain::types::FeedId;
use crate::domain::types::Timestamp;

/// Options for controlling merge behavior.
#[derive(Debug, Clone)]
pub struct MergeOptions {
    /// Maximum millisecond difference to consider two trades as duplicates
    pub dedup_tolerance_ms: u64,
    /// Minimum gap duration (ms) between trades before flagging as a data gap
    pub min_gap_duration_ms: u64,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            dedup_tolerance_ms: 1,
            min_gap_duration_ms: 5_000,
        }
    }
}

/// Merges data segments from multiple feeds into a single sorted,
/// deduplicated trade list with gap detection, using default options.
pub fn merge_segments(
    segments: Vec<DataSegment>,
    expected_start: Timestamp,
    expected_end: Timestamp,
) -> MergeResult {
    merge_segments_with(
        segments,
        expected_start,
        expected_end,
        &MergeOptions::default(),
    )
}

/// Merges data segments with explicit [`MergeOptions`] for dedup tolerance and gap detection.
pub fn merge_segments_with(
    mut segments: Vec<DataSegment>,
    expected_start: Timestamp,
    expected_end: Timestamp,
    opts: &MergeOptions,
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

    let mut feed_ids: Vec<FeedId> = segments.iter().map(|s| s.feed_id).collect();
    feed_ids.sort();
    feed_ids.dedup();

    let total_trades: usize = segments.iter().map(|s| s.trades.len()).sum();
    let mut all_trades = Vec::with_capacity(total_trades);

    for segment in segments.drain(..) {
        all_trades.extend(segment.trades);
    }

    all_trades.sort_by(|a, b| {
        a.time
            .0
            .cmp(&b.time.0)
            .then_with(|| a.price.units().cmp(&b.price.units()))
    });

    let trades = dedup_trades(all_trades, opts.dedup_tolerance_ms);
    let gaps = detect_gaps(
        &trades,
        expected_start,
        expected_end,
        opts.min_gap_duration_ms,
    );

    MergeResult {
        trades,
        gaps,
        feed_ids,
    }
}

/// Removes duplicate trades that are within `tolerance_ms` and at the same price.
fn dedup_trades(trades: Vec<crate::domain::Trade>, tolerance_ms: u64) -> Vec<crate::domain::Trade> {
    if trades.len() <= 1 {
        return trades;
    }

    let mut result = Vec::with_capacity(trades.len());
    result.push(trades[0]);

    for trade in trades.into_iter().skip(1) {
        let last = result.last().unwrap();
        let time_diff = trade.time.0.abs_diff(last.time.0);
        let same_price = trade.price.units() == last.price.units();

        if time_diff <= tolerance_ms && same_price {
            continue;
        }

        result.push(trade);
    }

    result
}

/// Detects gaps in trade data relative to the expected time range.
fn detect_gaps(
    trades: &[crate::domain::Trade],
    expected_start: Timestamp,
    expected_end: Timestamp,
    min_gap_ms: u64,
) -> Vec<DataGap> {
    let mut gaps = Vec::new();

    if trades.is_empty() {
        if expected_end.0 > expected_start.0 + min_gap_ms {
            gaps.push(DataGap::new(
                expected_start,
                expected_end,
                DataGapKind::NoData,
            ));
        }
        return gaps;
    }

    let first_time = trades[0].time;
    if first_time.0 > expected_start.0 + min_gap_ms {
        gaps.push(DataGap::new(
            expected_start,
            first_time,
            DataGapKind::NoData,
        ));
    }

    for window in trades.windows(2) {
        let gap_ms = window[1].time.0.saturating_sub(window[0].time.0);
        if gap_ms > min_gap_ms {
            gaps.push(DataGap::new(
                window[0].time,
                window[1].time,
                DataGapKind::NoData,
            ));
        }
    }

    let last_time = trades[trades.len() - 1].time;
    if expected_end.0 > last_time.0 + min_gap_ms {
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
                make_trade(2001, 101.0), // within tolerance = dup
                make_trade(3000, 103.0), // different price = kept
            ],
        };

        let result = merge_segments(vec![seg_a, seg_b], Timestamp(1000), Timestamp(3000));
        assert_eq!(result.trades.len(), 4);
    }
}
