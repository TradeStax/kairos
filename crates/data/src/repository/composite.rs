//! Composite Trade Repository
//!
//! Wraps multiple TradeRepository implementations and merges their results.
//! Uses the FeedMerger to deduplicate overlapping data and detect gaps.

use crate::domain::chart::{DataSegment, MergeResult};
use crate::domain::types::Timestamp;
use crate::domain::{DateRange, FuturesTicker, Trade};
use crate::feed::FeedId;
use crate::repository::traits::{
    RepositoryError, RepositoryResult, RepositoryStats, TradeRepository,
};
use crate::services::feed_merger;
use std::sync::Arc;

/// A trade repository entry: feed id + repository instance
pub struct FeedRepo {
    pub feed_id: FeedId,
    pub repo: Arc<dyn TradeRepository>,
}

/// Composite repository that fetches from multiple underlying repositories
/// and merges the results.
pub struct CompositeTradeRepository {
    repos: Vec<FeedRepo>,
}

impl CompositeTradeRepository {
    /// Create a new composite repository from a list of (feed_id, repo) pairs.
    /// Repos should be ordered by priority (first = highest priority).
    pub fn new(repos: Vec<FeedRepo>) -> Self {
        Self { repos }
    }

    /// Fetch from all repos and merge
    pub async fn get_merged_trades(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<MergeResult> {
        let mut segments = Vec::with_capacity(self.repos.len());

        let mut errors = Vec::new();

        for feed_repo in &self.repos {
            match feed_repo.repo.get_trades(ticker, date_range).await {
                Ok(trades) => {
                    if !trades.is_empty() {
                        let start = trades.first().map(|t| t.time).unwrap_or(Timestamp(0));
                        let end = trades.last().map(|t| t.time).unwrap_or(Timestamp(0));

                        segments.push(DataSegment {
                            feed_id: feed_repo.feed_id,
                            start,
                            end,
                            trades,
                        });
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Feed {} failed to get trades for {}: {}",
                        feed_repo.feed_id,
                        ticker.as_str(),
                        e
                    );
                    errors.push(e);
                }
            }
        }

        // If all feeds failed, propagate the error
        if segments.is_empty() && !errors.is_empty() {
            return Err(errors.remove(0));
        }

        let expected_start = Timestamp(date_range.start_timestamp_ms());
        let expected_end = Timestamp(date_range.end_timestamp_ms());

        Ok(feed_merger::merge_segments(
            segments,
            expected_start,
            expected_end,
        ))
    }
}

#[async_trait::async_trait]
impl TradeRepository for CompositeTradeRepository {
    async fn get_trades(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<Trade>> {
        let merge_result = self.get_merged_trades(ticker, date_range).await?;
        Ok(merge_result.trades)
    }

    async fn has_trades(
        &self,
        ticker: &FuturesTicker,
        date: chrono::NaiveDate,
    ) -> RepositoryResult<bool> {
        for feed_repo in &self.repos {
            match feed_repo.repo.has_trades(ticker, date).await {
                Ok(true) => return Ok(true),
                Ok(false) => continue,
                Err(e) => {
                    log::debug!(
                        "Feed {} check failed for {}: {}",
                        feed_repo.feed_id,
                        ticker.as_str(),
                        e
                    );
                    continue;
                }
            }
        }
        Ok(false)
    }

    async fn get_trades_for_date(
        &self,
        ticker: &FuturesTicker,
        date: chrono::NaiveDate,
    ) -> RepositoryResult<Vec<Trade>> {
        let date_range = DateRange::new(date, date)
            .expect("invariant: equal dates form a valid single-day range");
        self.get_trades(ticker, &date_range).await
    }

    async fn store_trades(
        &self,
        _ticker: &FuturesTicker,
        _date: chrono::NaiveDate,
        _trades: Vec<Trade>,
    ) -> RepositoryResult<()> {
        // Composite repo is read-only for merging
        Err(RepositoryError::InvalidData(
            "Cannot store trades to composite repository".to_string(),
        ))
    }

    async fn find_gaps(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<DateRange>> {
        // Use first repo's gap finding as baseline
        if let Some(first) = self.repos.first() {
            first.repo.find_gaps(ticker, date_range).await
        } else {
            Ok(vec![*date_range])
        }
    }

    async fn stats(&self) -> RepositoryResult<RepositoryStats> {
        // Aggregate stats from all repos
        let mut combined = RepositoryStats::new();
        for feed_repo in &self.repos {
            if let Ok(stats) = feed_repo.repo.stats().await {
                combined.cached_days += stats.cached_days;
                combined.total_size += stats.total_size;
                combined.hits += stats.hits;
                combined.misses += stats.misses;
            }
        }
        let total = combined.hits + combined.misses;
        if total > 0 {
            combined.hit_rate = combined.hits as f64 / total as f64;
        }
        Ok(combined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{Price, Quantity, Side};

    struct MockRepo {
        trades: Vec<Trade>,
    }

    #[async_trait::async_trait]
    impl TradeRepository for MockRepo {
        async fn get_trades(
            &self,
            _ticker: &FuturesTicker,
            _date_range: &DateRange,
        ) -> RepositoryResult<Vec<Trade>> {
            Ok(self.trades.clone())
        }

        async fn has_trades(
            &self,
            _ticker: &FuturesTicker,
            _date: chrono::NaiveDate,
        ) -> RepositoryResult<bool> {
            Ok(!self.trades.is_empty())
        }

        async fn get_trades_for_date(
            &self,
            _ticker: &FuturesTicker,
            _date: chrono::NaiveDate,
        ) -> RepositoryResult<Vec<Trade>> {
            Ok(self.trades.clone())
        }

        async fn store_trades(
            &self,
            _ticker: &FuturesTicker,
            _date: chrono::NaiveDate,
            _trades: Vec<Trade>,
        ) -> RepositoryResult<()> {
            Ok(())
        }

        async fn find_gaps(
            &self,
            _ticker: &FuturesTicker,
            _date_range: &DateRange,
        ) -> RepositoryResult<Vec<DateRange>> {
            Ok(Vec::new())
        }

        async fn stats(&self) -> RepositoryResult<RepositoryStats> {
            Ok(RepositoryStats::new())
        }
    }

    fn make_trade(time_ms: u64, price: f32) -> Trade {
        Trade::new(
            Timestamp(time_ms),
            Price::from_f32(price),
            Quantity(1.0),
            Side::Buy,
        )
    }

    #[tokio::test]
    async fn test_composite_merge() {
        let repo_a = Arc::new(MockRepo {
            trades: vec![make_trade(1000, 100.0), make_trade(2000, 101.0)],
        });

        let repo_b = Arc::new(MockRepo {
            trades: vec![
                make_trade(1000, 100.0), // duplicate
                make_trade(1500, 100.5), // unique
            ],
        });

        let composite = CompositeTradeRepository::new(vec![
            FeedRepo {
                feed_id: FeedId::new_v4(),
                repo: repo_a,
            },
            FeedRepo {
                feed_id: FeedId::new_v4(),
                repo: repo_b,
            },
        ]);

        let ticker = FuturesTicker::new("ES.c.0", crate::domain::FuturesVenue::CMEGlobex);
        let date_range = DateRange::new(
            chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
        )
        .expect("invariant: equal dates form a valid single-day range");

        let result = composite
            .get_merged_trades(&ticker, &date_range)
            .await
            .unwrap();

        // Should have 3 unique trades: 1000@100, 1500@100.5, 2000@101
        assert_eq!(result.trades.len(), 3);
        assert_eq!(result.feed_ids.len(), 2);
    }
}
