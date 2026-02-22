//! Data Index — canonical source of truth for available data ranges.
//!
//! `DataIndex` is built by scanning connected data stores (cache directories,
//! live feeds) and provides a single `resolve_chart_range()` method that
//! replaces all ad-hoc range lookups.

use crate::domain::chart::ChartType;
use crate::domain::types::DateRange;
use crate::feed::FeedId;
use chrono::NaiveDate;
use std::collections::{BTreeSet, HashMap};

/// Identifies a data series in the index.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataKey {
    pub ticker: String,
    pub schema: String,
}

/// One feed's contribution to a `DataKey`.
#[derive(Debug, Clone)]
pub struct FeedContribution {
    pub feed_id: FeedId,
    pub dates: BTreeSet<NaiveDate>,
    pub has_realtime: bool,
}

/// Aggregated index of all data available through connected feeds.
#[derive(Debug, Clone, Default)]
pub struct DataIndex {
    entries: HashMap<DataKey, Vec<FeedContribution>>,
}

impl DataIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a feed's contribution for a particular data key.
    pub fn add_contribution(
        &mut self,
        key: DataKey,
        feed_id: FeedId,
        dates: BTreeSet<NaiveDate>,
        has_realtime: bool,
    ) {
        let contributions = self.entries.entry(key).or_default();
        // Update existing contribution for this feed, or insert new
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

    /// Remove all contributions from a specific feed.
    pub fn remove_feed(&mut self, feed_id: FeedId) {
        for contributions in self.entries.values_mut() {
            contributions.retain(|c| c.feed_id != feed_id);
        }
        // Remove empty keys
        self.entries.retain(|_, v| !v.is_empty());
    }

    /// Merge another index into this one (used after async scan).
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

    /// Single canonical range resolution for a chart.
    ///
    /// - Unions all dates across feeds for the ticker's trade data
    /// - If any feed has realtime, extends end to today
    /// - For heatmaps, truncates to the most recent 1 day
    pub fn resolve_chart_range(
        &self,
        ticker: &str,
        chart_type: ChartType,
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

        if all_dates.is_empty() {
            // No cached dates at all — only useful if a realtime feed
            // is present, in which case range is just today.
            if any_realtime {
                let today = chrono::Utc::now().date_naive();
                return DateRange::new(today, today).ok();
            }
            return None;
        }

        let start = *all_dates.iter().next().unwrap();
        let mut end = *all_dates.iter().next_back().unwrap();

        // If any feed provides realtime, extend end to today
        if any_realtime {
            let today = chrono::Utc::now().date_naive();
            if today > end {
                end = today;
            }
        }

        // For heatmaps, truncate to most recent 1 day
        if chart_type == ChartType::Heatmap {
            return DateRange::new(end, end).ok();
        }

        DateRange::new(start, end).ok()
    }

    /// All tickers that have trade data available.
    pub fn available_tickers(&self) -> Vec<String> {
        self.entries
            .keys()
            .filter(|k| k.schema == "trades")
            .map(|k| k.ticker.clone())
            .collect()
    }

    /// Get date ranges for all tickers (for display in the UI).
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
            {
                if let Ok(range) = DateRange::new(start, end) {
                    result.insert(key.ticker.clone(), range);
                }
            }
        }
        result
    }

    /// Check if any trade data is available for a ticker.
    pub fn has_data(&self, ticker: &str) -> bool {
        let key = DataKey {
            ticker: ticker.to_string(),
            schema: "trades".to_string(),
        };
        self.entries
            .get(&key)
            .is_some_and(|contributions| !contributions.is_empty())
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
            .resolve_chart_range("ES.c.0", ChartType::Candlestick)
            .unwrap();
        assert_eq!(range.start, date(2025, 1, 10));
        assert_eq!(range.end, date(2025, 1, 12));
    }

    #[test]
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
            .resolve_chart_range("ES.c.0", ChartType::Heatmap)
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
            .resolve_chart_range("ES.c.0", ChartType::Candlestick)
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
            .resolve_chart_range("NQ.c.0", ChartType::Candlestick)
            .unwrap();
        assert_eq!(range.start, date(2025, 1, 10));
        assert_eq!(range.end, date(2025, 1, 12));
    }

    #[test]
    fn test_available_tickers() {
        let mut index = DataIndex::new();
        let feed_id = uuid::Uuid::new_v4();

        let mut dates = BTreeSet::new();
        dates.insert(date(2025, 1, 10));

        index.add_contribution(
            DataKey {
                ticker: "ES.c.0".into(),
                schema: "trades".into(),
            },
            feed_id,
            dates.clone(),
            false,
        );
        index.add_contribution(
            DataKey {
                ticker: "ES.c.0".into(),
                schema: "mbp10".into(),
            },
            feed_id,
            dates.clone(),
            false,
        );
        index.add_contribution(
            DataKey {
                ticker: "NQ.c.0".into(),
                schema: "trades".into(),
            },
            feed_id,
            dates,
            false,
        );

        let mut tickers = index.available_tickers();
        tickers.sort();
        assert_eq!(tickers, vec!["ES.c.0", "NQ.c.0"]);
    }

    #[test]
    fn test_no_data_returns_none() {
        let index = DataIndex::new();
        assert!(index.resolve_chart_range("ES.c.0", ChartType::Candlestick).is_none());
        assert!(!index.has_data("ES.c.0"));
    }
}
