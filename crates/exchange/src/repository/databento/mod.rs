pub mod depth;
mod download;
pub mod mapper;
pub mod trades;

pub use depth::DatabentoDepthRepository;
pub use trades::DatabentoTradeRepository;

use crate::adapter::databento::cache::CacheManager;
use databento::dbn::Schema;
use kairos_data::domain::DateRange;
use kairos_data::repository::RepositoryResult;

/// Find cache gaps for a given schema without acquiring the manager lock.
///
/// Iterates dates in `date_range`, grouping consecutive uncached days into
/// `DateRange` values. Used by both `DatabentoTradeRepository` (Schema::Trades)
/// and `DatabentoDepthRepository` (Schema::Mbp10).
pub(super) async fn find_cache_gaps(
    cache: &CacheManager,
    symbol: &str,
    schema: Schema,
    date_range: &DateRange,
) -> RepositoryResult<Vec<DateRange>> {
    let mut gaps = Vec::new();
    let mut current = date_range.start;

    while current <= date_range.end {
        if !cache.has_cached(symbol, schema, current).await {
            let gap_start = current;
            let mut gap_end = current;

            while gap_end <= date_range.end
                && !cache.has_cached(symbol, schema, gap_end).await
            {
                gap_end += chrono::Duration::days(1);
            }

            gap_end -= chrono::Duration::days(1);
            if let Ok(range) = DateRange::new(gap_start, gap_end) {
                gaps.push(range);
            }
            current = gap_end + chrono::Duration::days(1);
        } else {
            current += chrono::Duration::days(1);
        }
    }

    Ok(gaps)
}

