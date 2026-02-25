//! Rithmic R|Protocol layer
//!
//! Low-level WebSocket connection, protobuf message encoding/decoding,
//! and plant actor infrastructure. Moved from the standalone `rithmic-rs`
//! crate — only the subset needed by Kairos is retained.

pub mod config;
pub mod messages;
pub mod ping;
pub mod request;
pub mod response;
#[allow(clippy::all)]
pub mod rti;
pub mod sender;
pub mod ws;

// Re-export commonly used types
pub use config::{ConfigError, RithmicConnectionConfig, RithmicEnv};
pub use messages::RithmicMessage;
pub use response::RithmicResponse;
pub use ws::ConnectStrategy;
