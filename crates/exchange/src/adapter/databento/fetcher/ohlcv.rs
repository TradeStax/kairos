//! OHLCV fetch methods for HistoricalDataManager

use super::HistoricalDataManager;
use super::aggregation::aggregate_to_timeframe;
use crate::adapter::databento::DatabentoError;
use crate::{Kline, Timeframe};
use databento::dbn::Schema;

impl HistoricalDataManager {
    /// Fetch OHLCV data with SMART gap-based batching AND progress reporting
    ///
    /// Identifies gaps in cached data and fetches them in batched requests,
    /// then saves each day individually for future reuse.
    ///
    /// Progress is logged at each step for UI tracking.
    pub async fn fetch_ohlcv(
        &mut self,
        symbol: &str,
        timeframe: Timeframe,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<Kline>, DatabentoError> {
        let (start, end) = range;
        let start_date = start.date_naive();
        let end_date = end.date_naive();

        // Always use OHLCV-1M for caching (most granular, can aggregate to any timeframe)
        let schema = Schema::Ohlcv1M;

        // Steps 1-3: Ensure all days are cached (fail-fast on fetch error)
        let cached_days =
            self.ensure_cached("fetch_ohlcv", symbol, schema, start_date, end_date)
                .await?;

        // Verify coverage — all requested days must be cached
        let total_days = (end_date - start_date).num_days() + 1;
        if cached_days.len() < total_days as usize {
            return Err(DatabentoError::Cache(format!(
                "Incomplete coverage for {}: only {}/{} days cached",
                symbol,
                cached_days.len(),
                total_days
            )));
        }

        // Step 4: Load all days from cache and assemble
        let mut all_klines = Vec::new();
        let mut current = start_date;
        let mut day_count = 0;

        while current <= end_date {
            day_count += 1;
            match self.load_day_from_cache(symbol, schema, current).await {
                Ok(day_klines) => {
                    all_klines.extend(day_klines);
                    if day_count % 5 == 0 || day_count == total_days as usize {
                        log::debug!(
                            "Progress: {}/{} days loaded, {} bars total",
                            day_count,
                            total_days,
                            all_klines.len()
                        );
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Could not load {} for {}: {:?}",
                        symbol,
                        current,
                        e
                    );
                }
            }
            current += chrono::Duration::days(1);
        }

        if all_klines.is_empty() {
            return Err(DatabentoError::SymbolNotFound(format!(
                "No data found for {} in range {} to {}",
                symbol, start_date, end_date
            )));
        }

        // Step 5: Filter to exact datetime range
        let filtered: Vec<_> = all_klines
            .into_iter()
            .filter(|k| {
                let ktime = k.time as i64;
                let sms = start.timestamp_millis();
                let ems = end.timestamp_millis();
                ktime >= sms && ktime <= ems
            })
            .collect();

        // Step 6: Aggregate to target timeframe if needed
        log::debug!(
            "Before aggregation: {} 1M bars for {:?} target",
            filtered.len(),
            timeframe
        );

        let final_klines = aggregate_to_timeframe(filtered, timeframe);

        log::info!(
            "fetch_ohlcv complete: {} {:?} bars for {}",
            final_klines.len(),
            timeframe,
            symbol
        );

        // Log first few timestamps to verify spacing
        if final_klines.len() > 3 {
            let interval = timeframe.to_milliseconds();
            log::debug!("First bar: t={}", final_klines[0].time);
            log::debug!(
                "Second bar: t={} (delta={}ms, expected={}ms)",
                final_klines[1].time,
                final_klines[1].time - final_klines[0].time,
                interval
            );
            log::debug!(
                "Third bar: t={} (delta={}ms)",
                final_klines[2].time,
                final_klines[2].time - final_klines[1].time
            );
        }

        Ok(final_klines)
    }
}
