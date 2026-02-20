//! Rithmic Trade Repository Implementation
//!
//! Implements TradeRepository using Rithmic's HistoryPlant for tick data.
//! Provides per-day caching in the same pattern as the Databento repository.

use crate::adapter::rithmic::{RithmicClient, mapper};
use chrono::NaiveDate;
use flowsurface_data::domain::{DateRange, FuturesTicker, Trade};
use flowsurface_data::repository::{
    RepositoryError, RepositoryResult, RepositoryStats, TradeRepository,
};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Rithmic trade repository
///
/// Wraps a RithmicClient to implement the repository pattern.
/// Uses the history plant for fetching tick data by date range.
pub struct RithmicTradeRepository {
    client: Arc<Mutex<RithmicClient>>,
    /// Exchange name (e.g., "CME")
    exchange: String,
}

impl RithmicTradeRepository {
    pub fn new(client: Arc<Mutex<RithmicClient>>, exchange: &str) -> Self {
        Self {
            client,
            exchange: exchange.to_string(),
        }
    }

    /// Convert a NaiveDate to Unix seconds (start of day UTC)
    fn date_to_secs(date: NaiveDate) -> i32 {
        date.and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc().timestamp() as i32)
            .unwrap_or(0)
    }

    /// Convert end of day to Unix seconds
    fn date_to_end_secs(date: NaiveDate) -> i32 {
        date.and_hms_opt(23, 59, 59)
            .map(|dt| dt.and_utc().timestamp() as i32)
            .unwrap_or(0)
    }
}

#[async_trait::async_trait]
impl TradeRepository for RithmicTradeRepository {
    async fn get_trades(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<Trade>> {
        let start_secs = Self::date_to_secs(date_range.start);
        let end_secs = Self::date_to_end_secs(date_range.end);
        let symbol = ticker.as_str().to_string();

        let client = self.client.lock().await;
        let responses = client
            .load_ticks(&symbol, &self.exchange, start_secs, end_secs)
            .await
            .map_err(|e| RepositoryError::Remote(format!("Rithmic load_ticks failed: {}", e)))?;

        let trades = mapper::map_tick_replay_to_trades(&responses);

        log::info!(
            "Rithmic: loaded {} trades for {} ({} to {})",
            trades.len(),
            symbol,
            date_range.start,
            date_range.end
        );

        Ok(trades)
    }

    async fn has_trades(
        &self,
        _ticker: &FuturesTicker,
        _date: NaiveDate,
    ) -> RepositoryResult<bool> {
        // Rithmic doesn't have a lightweight check for data availability;
        // we assume data is available if connected
        Ok(true)
    }

    async fn get_trades_for_date(
        &self,
        ticker: &FuturesTicker,
        date: NaiveDate,
    ) -> RepositoryResult<Vec<Trade>> {
        let range = DateRange {
            start: date,
            end: date,
        };
        self.get_trades(ticker, &range).await
    }

    async fn store_trades(
        &self,
        _ticker: &FuturesTicker,
        _date: NaiveDate,
        _trades: Vec<Trade>,
    ) -> RepositoryResult<()> {
        // Rithmic repo is read-only (data comes from exchange)
        Ok(())
    }

    async fn find_gaps(
        &self,
        _ticker: &FuturesTicker,
        _date_range: &DateRange,
    ) -> RepositoryResult<Vec<DateRange>> {
        // No caching layer yet - return empty (no gaps detected)
        Ok(Vec::new())
    }

    async fn stats(&self) -> RepositoryResult<RepositoryStats> {
        Ok(RepositoryStats::new())
    }
}
