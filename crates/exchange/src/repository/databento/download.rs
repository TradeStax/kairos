//! DownloadRepository implementation for DatabentoTradeRepository

use super::trades::{schema_from_discriminant, DatabentoTradeRepository};
use crate::adapter::databento::mapper;
use databento::historical::metadata::GetCostParams;
use kairos_data::domain::{DateRange, FuturesTicker};
use kairos_data::repository::{
    CacheCoverageReport, RepositoryError, RepositoryResult,
};
use time::OffsetDateTime;

#[async_trait::async_trait]
impl kairos_data::repository::DownloadRepository for DatabentoTradeRepository {
    /// Check cache coverage without acquiring the manager lock.
    async fn check_cache_coverage(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
    ) -> RepositoryResult<CacheCoverageReport> {
        let schema = schema_from_discriminant(schema_discriminant)?;
        let symbol = ticker.as_str();

        let mut cached_days = Vec::new();
        let mut uncached_days = Vec::new();

        for date in date_range.dates() {
            if self.cache.has_cached(symbol, schema, date).await {
                cached_days.push(date);
            } else {
                uncached_days.push(date);
            }
        }

        // Find consecutive gaps
        let mut gaps = Vec::new();
        if !uncached_days.is_empty() {
            let mut gap_start = uncached_days[0];
            let mut gap_end = uncached_days[0];

            for (i, &date) in uncached_days.iter().enumerate().skip(1) {
                if date == gap_end + chrono::Duration::days(1) {
                    gap_end = date;
                } else {
                    gaps.push((gap_start, gap_end));
                    gap_start = date;
                    gap_end = date;
                }

                if i == uncached_days.len() - 1 {
                    gaps.push((gap_start, gap_end));
                }
            }

            if uncached_days.len() == 1 {
                gaps.push((gap_start, gap_end));
            }
        }

        Ok(CacheCoverageReport {
            cached_count: cached_days.len(),
            uncached_count: uncached_days.len(),
            gaps,
            cached_dates: cached_days,
        })
    }

    async fn prefetch_to_cache(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
    ) -> RepositoryResult<usize> {
        let schema = schema_from_discriminant(schema_discriminant)?;
        let mut manager = self.manager.lock().await;
        let symbol = ticker.as_str();

        let mut downloaded = 0;

        for date in date_range.dates() {
            if !manager.cache.has_cached(symbol, schema, date).await {
                log::debug!("Downloading {} for {} (schema: {:?})", date, symbol, schema);

                manager
                    .fetch_to_cache(symbol, schema, date)
                    .await
                    .map_err(|e| {
                        RepositoryError::Remote(format!("Download failed for {}: {:?}", date, e))
                    })?;

                downloaded += 1;
                log::debug!("Successfully cached {}/{} for {}", date, schema, symbol);
            }
        }

        log::info!(
            "Prefetch complete: Downloaded {} days for {} ({:?})",
            downloaded,
            symbol,
            schema
        );

        Ok(downloaded)
    }

    async fn prefetch_to_cache_with_progress(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
        progress_callback: Box<dyn Fn(usize, usize) + Send + Sync>,
    ) -> RepositoryResult<usize> {
        let schema = schema_from_discriminant(schema_discriminant)?;
        let mut manager = self.manager.lock().await;
        let symbol = ticker.as_str();

        let total_days = date_range.num_days() as usize;
        let mut downloaded = 0;
        let mut processed = 0;

        log::debug!(
            "Starting prefetch with progress: {} days for {} ({:?})",
            total_days,
            symbol,
            schema
        );

        for date in date_range.dates() {
            if !manager.cache.has_cached(symbol, schema, date).await {
                log::debug!("Downloading {} for {} (schema: {:?})", date, symbol, schema);

                manager
                    .fetch_to_cache(symbol, schema, date)
                    .await
                    .map_err(|e| {
                        RepositoryError::Remote(format!("Download failed for {}: {:?}", date, e))
                    })?;

                downloaded += 1;
                log::debug!("Successfully cached {}/{} for {}", date, schema, symbol);
            } else {
                log::debug!("Skipping {} - already cached", date);
            }

            processed += 1;
            progress_callback(processed, total_days);
        }

        log::info!(
            "Prefetch complete: Downloaded {} days for {} ({:?})",
            downloaded,
            symbol,
            schema
        );

        Ok(downloaded)
    }

    async fn get_download_cost(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
    ) -> RepositoryResult<f64> {
        log::debug!("get_download_cost called for {:?}", ticker);

        let schema = schema_from_discriminant(schema_discriminant)?;
        let mut manager = self.manager.lock().await;
        let symbol = ticker.as_str();

        // Convert DateRange to chrono DateTime (UTC start/end of day)
        // NOTE: Databento API uses exclusive end times
        let start = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            date_range
                .start
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| RepositoryError::InvalidData("Invalid start date".to_string()))?,
            chrono::Utc,
        );
        let end = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            (date_range.end + chrono::Duration::days(1))
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| RepositoryError::InvalidData("Invalid end date".to_string()))?,
            chrono::Utc,
        );

        let start_time = OffsetDateTime::from_unix_timestamp(start.timestamp())
            .map_err(|e| RepositoryError::InvalidData(format!("Invalid start time: {}", e)))?;
        let end_time = OffsetDateTime::from_unix_timestamp(end.timestamp())
            .map_err(|e| RepositoryError::InvalidData(format!("Invalid end time: {}", e)))?;

        let stype = mapper::determine_stype(symbol);

        let cost_params = GetCostParams::builder()
            .dataset(manager.config.dataset)
            .symbols(vec![symbol])
            .schema(schema)
            .stype_in(stype)
            .date_time_range((start_time, end_time))
            .build();

        log::info!(
            "Calling Databento cost API: symbol={}, schema={:?}, range={:?} to {:?}",
            symbol,
            schema,
            date_range.start,
            date_range.end
        );

        match manager.client.metadata().get_cost(&cost_params).await {
            Ok(cost_usd) => {
                log::info!(
                    "Databento cost API: ${:.4} USD for {} from {} to {}",
                    cost_usd,
                    symbol,
                    date_range.start,
                    date_range.end
                );
                Ok(cost_usd)
            }
            Err(e) => {
                log::error!("Databento cost API failed: {:?}", e);
                Err(RepositoryError::Remote(format!(
                    "Databento cost API failed: {:?}",
                    e
                )))
            }
        }
    }

    /// List cached symbols without acquiring the manager lock.
    async fn list_cached_symbols(
        &self,
    ) -> RepositoryResult<std::collections::HashSet<String>> {
        self.cache.list_cached_symbols().await.map_err(|e| {
            RepositoryError::Cache(format!("Failed to list cached symbols: {:?}", e))
        })
    }
}
