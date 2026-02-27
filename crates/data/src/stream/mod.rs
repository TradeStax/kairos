//! Stream subscription types with a two-tier serialization model.
//!
//! - [`PersistStreamKind`] — serializable config stored in layout state (ticker symbol + params)
//! - [`StreamKind`] — runtime-resolved with full [`FuturesTickerInfo`](crate::domain::FuturesTickerInfo)
//!
//! [`ResolvedStream`] wraps the transition: `Waiting` before ticker info is available,
//! `Ready` after resolution. [`UniqueStreams`] deduplicates across panes.

pub mod kind;
pub mod resolved;
pub mod schema;
pub mod unique;

#[cfg(feature = "heatmap")]
pub use kind::PersistDepth;
pub use kind::{PersistKline, PersistStreamKind, PushFrequency, StreamKind, StreamTicksize};
pub use resolved::ResolvedStream;
pub use schema::DownloadSchema;
pub use unique::{StreamConfig, StreamSpecs, UniqueStreams};
