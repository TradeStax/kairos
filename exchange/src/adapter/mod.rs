//! Exchange adapters for market data sources.
//!
//! Each adapter handles connection, fetching, caching, and type conversion
//! for a specific data provider:
//! - [`databento`] - Historical CME futures data
//! - [`massive`] - US options data via Polygon
//! - [`rithmic`] - Real-time CME futures streaming
//!
//! Common types are split into focused sub-modules:
//! - [`stream`] - Stream types, persistence, and configuration
//! - [`event`] - Events emitted by adapters (historical and live)
//! - [`error`] - Adapter-level error type

pub mod databento;
pub mod error;
pub mod event;
pub mod massive;
pub mod rithmic;
pub mod stream;

pub use error::AdapterError;
pub use event::Event;
pub use stream::{
    PersistDepth, PersistKline, PersistStreamKind, ResolvedStream, StreamConfig, StreamKind,
    StreamSpecs, StreamTicksize, UniqueStreams,
};
