//! Rithmic R|Protocol layer.
//!
//! Low-level WebSocket connection management, protobuf message
//! encoding/decoding, request/response correlation, and plant actor
//! infrastructure. Derived from the standalone `rithmic-rs` crate --
//! only the subset needed by Kairos is retained.

pub mod config;
pub mod messages;
pub mod ping;
pub mod request;
pub mod response;
#[allow(clippy::all)]
pub mod rti;
pub mod sender;
pub mod ws;

pub use config::{RithmicConnectionConfig, RithmicEnv};
pub use messages::RithmicMessage;
pub use response::RithmicResponse;
pub use ws::ConnectStrategy;
