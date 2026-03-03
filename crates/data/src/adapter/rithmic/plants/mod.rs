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

use super::protocol::response::RithmicResponse;

/// Error message for dropped oneshot/mpsc channels (actor shut down).
pub(crate) const CONN_CLOSED_ERR: &str = "Connection closed";

/// Extract the first response from a `Vec`, returning an error if empty.
///
/// Many plant commands return `Vec<RithmicResponse>` where the caller
/// only needs the first element. This avoids panicking on `.remove(0)`
/// when the server sends an empty response set.
fn take_first(mut responses: Vec<RithmicResponse>) -> Result<RithmicResponse, String> {
    if responses.is_empty() {
        Err("Empty response from server".to_owned())
    } else {
        Ok(responses.swap_remove(0))
    }
}
