//! Per-day cache layer.
//!
//! Bincode+zstd storage for all adapters. Atomic writes (`.tmp` then rename).
//! Lock-free reads via async filesystem I/O.
//!
//! - [`store`] — `CacheStore` read/write/scan/evict operations, `CacheProvider`, `CacheSchema`
//! - [`mod@format`] — `DayFileHeader`, bincode+zstd encode/decode
//! - [`live_buffer`] — `LiveDayBuffer` accumulates streaming data before day-end flush
//! - [`stats`] — `CacheStats` for file count and size reporting

pub mod format;
pub mod live_buffer;
pub mod stats;
pub mod store;

pub use stats::CacheStats;
pub use store::{CacheProvider, CacheSchema, CacheStore};
