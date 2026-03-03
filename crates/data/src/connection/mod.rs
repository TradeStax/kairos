//! Connection configuration and lifecycle management for data feeds.
//!
//! - [`manager`] — `ConnectionManager` stores connections, resolves best source for a ticker
//! - [`types`] — `Connection`, `ConnectionProvider`, `ConnectionStatus`, `ConnectionCapability`
//! - [`config`] — `ConnectionConfig`, `DatabentoConnectionConfig`, `RithmicConnectionConfig`,
//!   `RithmicServer`, `RithmicEnvironment`

pub mod config;
pub mod manager;
pub mod server_resolver;
pub mod types;

pub use config::{
    ConnectionConfig, DatabentoConnectionConfig, RithmicConnectionConfig, RithmicEnvironment,
    RithmicServer,
};
pub use manager::{ConnectionManager, ResolvedConnection};
pub use server_resolver::ServerResolver;
pub use types::{
    Connection, ConnectionCapability, ConnectionKind, ConnectionProvider, ConnectionStatus,
    HistoricalDatasetInfo,
};
