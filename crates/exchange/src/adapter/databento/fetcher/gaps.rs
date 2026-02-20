//! Date range gap detection for cache-aware fetching

use std::collections::HashSet;

/// Date range gap (consecutive uncached days)
#[derive(Debug, Clone)]
pub(crate) struct DateGap {
    pub start: chrono::NaiveDate,
    pub end: chrono::NaiveDate,
}

/// Find consecutive gaps in cached days
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
            // This day is cached - close any open gap
            if let Some(gap_s) = gap_start {
                gaps.push(DateGap {
                    start: gap_s,
                    end: current - chrono::Duration::days(1),
                });
                gap_start = None;
            }
        } else {
            // This day is NOT cached - extend or start gap
            if gap_start.is_none() {
                gap_start = Some(current);
            }
        }
        current += chrono::Duration::days(1);
    }

    // Close final gap if exists
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

        // Cached: 5/10-5/13 + 5/16
        let mut cached = HashSet::new();
        cached.insert(NaiveDate::from_ymd_opt(2024, 5, 10).unwrap());
        cached.insert(NaiveDate::from_ymd_opt(2024, 5, 11).unwrap());
        cached.insert(NaiveDate::from_ymd_opt(2024, 5, 12).unwrap());
        cached.insert(NaiveDate::from_ymd_opt(2024, 5, 13).unwrap());
        cached.insert(NaiveDate::from_ymd_opt(2024, 5, 16).unwrap());

        let gaps = find_uncached_gaps((start, end), &cached);

        // Should find 3 gaps: [5/6-5/9], [5/14-5/15], [5/17-5/18]
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0].start, NaiveDate::from_ymd_opt(2024, 5, 6).unwrap());
        assert_eq!(gaps[0].end, NaiveDate::from_ymd_opt(2024, 5, 9).unwrap());
        assert_eq!(gaps[1].start, NaiveDate::from_ymd_opt(2024, 5, 14).unwrap());
        assert_eq!(gaps[1].end, NaiveDate::from_ymd_opt(2024, 5, 15).unwrap());
        assert_eq!(gaps[2].start, NaiveDate::from_ymd_opt(2024, 5, 17).unwrap());
        assert_eq!(gaps[2].end, NaiveDate::from_ymd_opt(2024, 5, 18).unwrap());
    }
}
