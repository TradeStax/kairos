//! Date range gap detection for cache-aware fetching.
//!
//! Given a requested date range and a set of already-cached dates, identifies
//! contiguous gaps that need to be fetched from the API.

use std::collections::HashSet;

/// A contiguous range of dates not present in the cache
#[derive(Debug, Clone)]
pub(crate) struct DateGap {
    /// First uncached date (inclusive)
    pub start: chrono::NaiveDate,
    /// Last uncached date (inclusive)
    pub end: chrono::NaiveDate,
}

/// Finds contiguous date gaps within `requested_range` that are not in `cached_days`.
///
/// Returns a list of [`DateGap`] values representing ranges that need fetching,
/// ordered chronologically. Adjacent uncached days are merged into a single gap.
pub(crate) fn find_uncached_gaps(
    requested_range: (chrono::NaiveDate, chrono::NaiveDate),
    cached_days: &HashSet<chrono::NaiveDate>,
) -> Vec<DateGap> {
    let (start, end) = requested_range;
    let mut gaps = Vec::new();
    let mut gap_start: Option<chrono::NaiveDate> = None;

    let mut current = start;
    while current <= end {
        if cached_days.contains(&current) {
            if let Some(gap_s) = gap_start {
                gaps.push(DateGap {
                    start: gap_s,
                    end: current - chrono::Duration::days(1),
                });
                gap_start = None;
            }
        } else if gap_start.is_none() {
            gap_start = Some(current);
        }
        current += chrono::Duration::days(1);
    }

    if let Some(gap_s) = gap_start {
        gaps.push(DateGap { start: gap_s, end });
    }

    gaps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gap_detection() {
        use chrono::NaiveDate;

        let start = NaiveDate::from_ymd_opt(2024, 5, 6).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 5, 18).unwrap();

        let mut cached = HashSet::new();
        for day in [10, 11, 12, 13, 16] {
            cached.insert(NaiveDate::from_ymd_opt(2024, 5, day).unwrap());
        }

        let gaps = find_uncached_gaps((start, end), &cached);

        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0].start, NaiveDate::from_ymd_opt(2024, 5, 6).unwrap());
        assert_eq!(gaps[0].end, NaiveDate::from_ymd_opt(2024, 5, 9).unwrap());
        assert_eq!(gaps[1].start, NaiveDate::from_ymd_opt(2024, 5, 14).unwrap());
        assert_eq!(gaps[1].end, NaiveDate::from_ymd_opt(2024, 5, 15).unwrap());
        assert_eq!(gaps[2].start, NaiveDate::from_ymd_opt(2024, 5, 17).unwrap());
        assert_eq!(gaps[2].end, NaiveDate::from_ymd_opt(2024, 5, 18).unwrap());
    }
}
