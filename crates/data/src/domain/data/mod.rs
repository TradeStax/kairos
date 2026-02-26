//! Data indexing and download tracking.
//!
//! - `DataIndex` — tracks available tickers, schemas, dates, and feed contributions.
//!   Built by scanning the cache directory on connect.
//! - `DownloadedTickersRegistry` — persisted record of explicitly downloaded ticker date ranges

pub mod index;
pub mod registry;

// Re-export commonly used types
pub use index::{DataIndex, DataKey, FeedContribution};
pub use registry::DownloadedTickersRegistry;
