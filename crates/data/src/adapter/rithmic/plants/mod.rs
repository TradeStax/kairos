//! Rithmic plant actor implementations.
//!
//! Each plant manages a WebSocket connection to a specific Rithmic
//! infrastructure type: [`ticker`] for real-time market data and
//! [`history`] for historical data retrieval. Both run as background
//! `tokio` tasks driven by a command channel.

pub mod history;
pub mod ticker;

pub use history::{RithmicHistoryPlant, RithmicHistoryPlantHandle};
pub use ticker::{RithmicTickerPlant, RithmicTickerPlantHandle};
