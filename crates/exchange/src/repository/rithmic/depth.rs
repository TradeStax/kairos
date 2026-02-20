//! Rithmic Depth Repository Implementation
//!
//! Implements DepthRepository for Rithmic depth data.
//! Currently a placeholder - real-time depth comes via streaming,
//! historical depth can be fetched from the history plant if needed.

use chrono::NaiveDate;
use kairos_data::domain::{DateRange, DepthSnapshot, FuturesTicker};
use kairos_data::repository::{
    DepthRepository, RepositoryError, RepositoryResult, RepositoryStats,
};

/// Rithmic depth repository
///
/// Depth data from Rithmic primarily comes via real-time streaming
/// (BBO and OrderBook messages). This repository provides the trait
/// implementation for historical depth queries.
#[derive(Default)]
pub struct RithmicDepthRepository {
    // Placeholder - will hold client reference when historical depth
    // fetching is implemented
}

impl RithmicDepthRepository {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl DepthRepository for RithmicDepthRepository {
    async fn get_depth(
        &self,
        _ticker: &FuturesTicker,
        _date_range: &DateRange,
    ) -> RepositoryResult<Vec<DepthSnapshot>> {
        Err(RepositoryError::NotFound(
            "Rithmic depth data is available via real-time streaming only. \
             Historical depth queries are not supported."
                .to_string(),
        ))
    }

    async fn has_depth(&self, _ticker: &FuturesTicker, _date: NaiveDate) -> RepositoryResult<bool> {
        Ok(false)
    }

    async fn get_depth_for_date(
        &self,
        _ticker: &FuturesTicker,
        _date: NaiveDate,
    ) -> RepositoryResult<Vec<DepthSnapshot>> {
        Err(RepositoryError::NotFound(
            "Rithmic depth data is available via real-time streaming only. \
             Historical depth queries are not supported."
                .to_string(),
        ))
    }

    async fn store_depth(
        &self,
        _ticker: &FuturesTicker,
        _date: NaiveDate,
        _depth: Vec<DepthSnapshot>,
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
