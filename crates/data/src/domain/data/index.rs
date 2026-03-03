//! Data index — canonical source of truth for what data ranges are
//! available across all connected feeds.

use std::collections::{BTreeSet, HashMap};

use chrono::NaiveDate;

use crate::domain::chart::config::ChartType;
use crate::domain::core::types::{DateRange, FeedId};

/// Identifies a data series in the index by ticker symbol and schema name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataKey {
    /// Ticker symbol (e.g. `"ES.c.0"`)
    pub ticker: String,
    /// Schema name (e.g. `"trades"`, `"mbp-10"`)
    pub schema: String,
}

/// One feed's contribution to a [`DataKey`].
#[derive(Debug, Clone)]
pub struct FeedContribution {
    /// Source feed identifier
    pub feed_id: FeedId,
    /// Calendar dates for which this feed has cached data
    pub dates: BTreeSet<NaiveDate>,
    /// Whether this feed provides live / real-time data
    pub has_realtime: bool,
}

/// Aggregated index of all data available through connected feeds.
///
/// Keyed by `(ticker, schema)` pairs, each entry stores the per-feed
/// date contributions. Used to resolve chart date ranges and drive the
/// data download UI.
#[derive(Debug, Clone, Default)]
pub struct DataIndex {
    entries: HashMap<DataKey, Vec<FeedContribution>>,
}

impl DataIndex {
    /// Create an empty index
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a feed's dates for a data key, merging with any existing contribution
    pub fn add_contribution(
        &mut self,
        key: DataKey,
        feed_id: FeedId,
        dates: BTreeSet<NaiveDate>,
        has_realtime: bool,
    ) {
        let contributions = self.entries.entry(key).or_default();
        if let Some(existing) = contributions.iter_mut().find(|c| c.feed_id == feed_id) {
            existing.dates.extend(dates);
            existing.has_realtime |= has_realtime;
        } else {
            contributions.push(FeedContribution {
                feed_id,
                dates,
                has_realtime,
            });
        }
    }

    /// Remove all contributions from a specific feed
    pub fn remove_feed(&mut self, feed_id: FeedId) {
        for contributions in self.entries.values_mut() {
            contributions.retain(|c| c.feed_id != feed_id);
        }
        self.entries.retain(|_, v| !v.is_empty());
    }

    /// Merge another index into this one
    pub fn merge(&mut self, other: DataIndex) {
        for (key, contributions) in other.entries {
            for contrib in contributions {
                self.add_contribution(
                    key.clone(),
                    contrib.feed_id,
                    contrib.dates,
                    contrib.has_realtime,
                );
            }
        }
    }

    /// Resolve the best available date range for a chart.
    ///
    /// Combines dates across all feeds and extends to today if any feed
    /// has real-time data. When `max_backfill_days` is `Some(N)` and a
    /// realtime feed is active, the start date is capped to `today - N`
    /// so cached data from earlier sessions doesn't inflate the range
    /// beyond the configured backfill window.
    ///
    /// For heatmap charts (with the `heatmap` feature), the range is
    /// truncated to a single day.
    #[must_use]
    pub fn resolve_chart_range(
        &self,
        ticker: &str,
        _chart_type: ChartType,
        max_backfill_days: Option<i64>,
    ) -> Option<DateRange> {
        let trade_key = DataKey {
            ticker: ticker.to_string(),
            schema: "trades".to_string(),
        };

        let contributions = self.entries.get(&trade_key)?;
        if contributions.is_empty() {
            return None;
        }

        let mut all_dates = BTreeSet::new();
        let mut any_realtime = false;

        for contrib in contributions {
            all_dates.extend(&contrib.dates);
            any_realtime |= contrib.has_realtime;
        }

        /// When no cached dates exist but a realtime feed is connected,
        /// fall back to this many days of history.
        const DEFAULT_REALTIME_FALLBACK_DAYS: i64 = 4;

        if all_dates.is_empty() {
            if any_realtime {
                let today = DateRange::today_et();
                let days = max_backfill_days.unwrap_or(DEFAULT_REALTIME_FALLBACK_DAYS);
                let start = today - chrono::Duration::days(days);
                return DateRange::new(start, today).ok();
            }
            return None;
        }

        let mut start = *all_dates.iter().next().expect("all_dates non-empty");
        let mut end = *all_dates.iter().next_back().expect("all_dates non-empty");

        if any_realtime {
            let today = DateRange::today_et();
            if today > end {
                end = today;
            }
            // Cap the start date so the backfill range doesn't exceed
            // the configured limit (e.g. backfill_days from feed config).
            // Without this, old cached dates would inflate the range
            // beyond what the user expects from their backfill setting.
            if let Some(days) = max_backfill_days {
                let min_start = today - chrono::Duration::days(days);
                if start < min_start {
                    start = min_start;
                }
            }
        }

        #[cfg(feature = "heatmap")]
        if _chart_type == ChartType::Heatmap {
            return DateRange::new(end, end).ok();
        }

        DateRange::new(start, end).ok()
    }

    /// Return all ticker symbols that have trade data
    #[must_use]
    pub fn available_tickers(&self) -> Vec<String> {
        self.entries
            .keys()
            .filter(|k| k.schema == "trades")
            .map(|k| k.ticker.clone())
            .collect()
    }

    /// Return a map of ticker symbols to their overall date ranges
    #[must_use]
    pub fn ticker_date_ranges(&self) -> HashMap<String, DateRange> {
        let mut result = HashMap::new();
        for (key, contributions) in &self.entries {
            if key.schema != "trades" {
                continue;
            }
            let mut all_dates = BTreeSet::new();
            for contrib in contributions {
                all_dates.extend(&contrib.dates);
            }
            if let (Some(&start), Some(&end)) =
                (all_dates.iter().next(), all_dates.iter().next_back())
                && let Ok(range) = DateRange::new(start, end)
            {
                result.insert(key.ticker.clone(), range);
            }
        }
        result
    }

    /// Return all dates with trade data for a ticker across all feeds
    #[must_use]
    pub fn available_dates(&self, ticker: &str) -> BTreeSet<NaiveDate> {
        let trade_key = DataKey {
            ticker: ticker.to_string(),
            schema: "trades".to_string(),
        };

        let Some(contributions) = self.entries.get(&trade_key) else {
            return BTreeSet::new();
        };

        let mut all_dates = BTreeSet::new();
        for contrib in contributions {
            all_dates.extend(&contrib.dates);
        }
        all_dates
    }

    /// Return `true` if any feed has trade data for the given ticker
    #[must_use]
    pub fn has_data(&self, ticker: &str) -> bool {
        let key = DataKey {
            ticker: ticker.to_string(),
            schema: "trades".to_string(),
        };
        self.entries
            .get(&key)
            .is_some_and(|contributions| !contributions.is_empty())
    }

    /// Return ticker symbols that have trade data from a specific feed
    #[must_use]
    pub fn tickers_for_feed(&self, feed_id: FeedId) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(key, contributions)| {
                key.schema == "trades" && contributions.iter().any(|c| c.feed_id == feed_id)
            })
            .map(|(key, _)| key.ticker.clone())
            .collect()
    }

    /// Return cached dates for a ticker from a specific feed
    #[must_use]
    pub fn available_dates_for_feed(&self, ticker: &str, feed_id: FeedId) -> BTreeSet<NaiveDate> {
        let trade_key = DataKey {
            ticker: ticker.to_string(),
            schema: "trades".to_string(),
        };
        self.entries
            .get(&trade_key)
            .into_iter()
            .flat_map(|contribs| contribs.iter())
            .filter(|c| c.feed_id == feed_id)
            .flat_map(|c| c.dates.iter().copied())
            .collect()
    }

    /// Return `true` if any contribution for the given feed has real-time data
    #[must_use]
    pub fn feed_has_realtime(&self, feed_id: FeedId) -> bool {
        self.entries.values().any(|contribs| {
            contribs
                .iter()
                .any(|c| c.feed_id == feed_id && c.has_realtime)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn test_add_and_resolve() {
        let mut index = DataIndex::new();
        let feed_id = uuid::Uuid::new_v4();

        let mut dates = BTreeSet::new();
        dates.insert(date(2025, 1, 10));
        dates.insert(date(2025, 1, 11));
        dates.insert(date(2025, 1, 12));

        index.add_contribution(
            DataKey {
                ticker: "ES.c.0".into(),
                schema: "trades".into(),
            },
            feed_id,
            dates,
            false,
        );

        let range = index
            .resolve_chart_range("ES.c.0", ChartType::Candlestick, None)
            .unwrap();
        assert_eq!(range.start, date(2025, 1, 10));
        assert_eq!(range.end, date(2025, 1, 12));
    }

    #[test]
    #[cfg(feature = "heatmap")]
    fn test_heatmap_truncation() {
        let mut index = DataIndex::new();
        let feed_id = uuid::Uuid::new_v4();

        let mut dates = BTreeSet::new();
        dates.insert(date(2025, 1, 10));
        dates.insert(date(2025, 1, 11));
        dates.insert(date(2025, 1, 12));

        index.add_contribution(
            DataKey {
                ticker: "ES.c.0".into(),
                schema: "trades".into(),
            },
            feed_id,
            dates,
            false,
        );

        let range = index
            .resolve_chart_range("ES.c.0", ChartType::Heatmap, None)
            .unwrap();
        assert_eq!(range.start, date(2025, 1, 12));
        assert_eq!(range.end, date(2025, 1, 12));
    }

    #[test]
    fn test_remove_feed() {
        let mut index = DataIndex::new();
        let feed_a = uuid::Uuid::new_v4();
        let feed_b = uuid::Uuid::new_v4();

        let mut dates_a = BTreeSet::new();
        dates_a.insert(date(2025, 1, 10));

        let mut dates_b = BTreeSet::new();
        dates_b.insert(date(2025, 1, 11));

        let key = DataKey {
            ticker: "ES.c.0".into(),
            schema: "trades".into(),
        };

        index.add_contribution(key.clone(), feed_a, dates_a, false);
        index.add_contribution(key, feed_b, dates_b, false);

        assert!(index.has_data("ES.c.0"));

        index.remove_feed(feed_a);
        let range = index
            .resolve_chart_range("ES.c.0", ChartType::Candlestick, None)
            .unwrap();
        assert_eq!(range.start, date(2025, 1, 11));
        assert_eq!(range.end, date(2025, 1, 11));

        index.remove_feed(feed_b);
        assert!(!index.has_data("ES.c.0"));
    }

    #[test]
    fn test_merge() {
        let mut index_a = DataIndex::new();
        let mut index_b = DataIndex::new();
        let feed_id = uuid::Uuid::new_v4();

        let key = DataKey {
            ticker: "NQ.c.0".into(),
            schema: "trades".into(),
        };

        let mut dates_a = BTreeSet::new();
        dates_a.insert(date(2025, 1, 10));
        index_a.add_contribution(key.clone(), feed_id, dates_a, false);

        let mut dates_b = BTreeSet::new();
        dates_b.insert(date(2025, 1, 12));
        index_b.add_contribution(key, feed_id, dates_b, false);

        index_a.merge(index_b);

        let range = index_a
            .resolve_chart_range("NQ.c.0", ChartType::Candlestick, None)
            .unwrap();
        assert_eq!(range.start, date(2025, 1, 10));
        assert_eq!(range.end, date(2025, 1, 12));
    }

    #[test]
    fn test_no_data_returns_none() {
        let index = DataIndex::new();
        assert!(
            index
                .resolve_chart_range("ES.c.0", ChartType::Candlestick, None)
                .is_none()
        );
        assert!(!index.has_data("ES.c.0"));
    }

    #[test]
    fn test_realtime_caps_start_to_backfill_days() {
        let mut index = DataIndex::new();
        let feed_id = uuid::Uuid::new_v4();

        // Add cached dates spanning a wide range (30+ days old)
        let today = DateRange::today_et();
        let mut dates = BTreeSet::new();
        for offset in [30, 25, 20, 15, 10, 5, 3, 1] {
            dates.insert(today - chrono::Duration::days(offset));
        }

        index.add_contribution(
            DataKey {
                ticker: "ES.c.0".into(),
                schema: "trades".into(),
            },
            feed_id,
            dates,
            true, // realtime feed
        );

        // With max_backfill_days=5, start should be capped to today-5
        let range = index
            .resolve_chart_range("ES.c.0", ChartType::Candlestick, Some(5))
            .unwrap();
        let expected_start = today - chrono::Duration::days(5);
        assert_eq!(range.start, expected_start);
        assert_eq!(range.end, today);

        // Without cap (None), the full cached range is used
        let range_uncapped = index
            .resolve_chart_range("ES.c.0", ChartType::Candlestick, None)
            .unwrap();
        assert_eq!(range_uncapped.start, today - chrono::Duration::days(30));
        assert_eq!(range_uncapped.end, today);
    }

    #[test]
    fn test_realtime_no_cache_uses_fallback() {
        let mut index = DataIndex::new();
        let feed_id = uuid::Uuid::new_v4();

        // Realtime feed with empty dates
        index.add_contribution(
            DataKey {
                ticker: "NQ.c.0".into(),
                schema: "trades".into(),
            },
            feed_id,
            BTreeSet::new(),
            true,
        );

        let today = DateRange::today_et();

        // With explicit backfill_days=7, uses that as fallback
        let range = index
            .resolve_chart_range("NQ.c.0", ChartType::Candlestick, Some(7))
            .unwrap();
        assert_eq!(range.start, today - chrono::Duration::days(7));
        assert_eq!(range.end, today);

        // Without backfill_days, uses default 4-day fallback
        let range_default = index
            .resolve_chart_range("NQ.c.0", ChartType::Candlestick, None)
            .unwrap();
        assert_eq!(range_default.start, today - chrono::Duration::days(4));
        assert_eq!(range_default.end, today);
    }

    #[test]
    fn test_available_dates() {
        let mut index = DataIndex::new();
        let feed_a = uuid::Uuid::new_v4();
        let feed_b = uuid::Uuid::new_v4();

        let mut dates_a = BTreeSet::new();
        dates_a.insert(date(2025, 1, 10));
        dates_a.insert(date(2025, 1, 11));

        let mut dates_b = BTreeSet::new();
        dates_b.insert(date(2025, 1, 11));
        dates_b.insert(date(2025, 1, 13));

        let key = DataKey {
            ticker: "ES.c.0".into(),
            schema: "trades".into(),
        };

        index.add_contribution(key.clone(), feed_a, dates_a, false);
        index.add_contribution(key, feed_b, dates_b, false);

        let dates = index.available_dates("ES.c.0");
        let expected: BTreeSet<_> = [date(2025, 1, 10), date(2025, 1, 11), date(2025, 1, 13)]
            .into_iter()
            .collect();
        assert_eq!(dates, expected);
        assert!(index.available_dates("ZZ.c.0").is_empty());
    }
}
