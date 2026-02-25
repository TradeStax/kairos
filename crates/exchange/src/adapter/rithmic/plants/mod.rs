//! Rithmic plant actor implementations
//!
//! Each plant manages a WebSocket connection to a specific Rithmic
//! infrastructure type (ticker for real-time data, history for
//! historical data retrieval).

pub mod history;
pub mod ticker;

pub use history::{RithmicHistoryPlant, RithmicHistoryPlantHandle};
pub use ticker::{RithmicTickerPlant, RithmicTickerPlantHandle};
