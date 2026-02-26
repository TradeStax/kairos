//! Replay playback state.
//!
//! - `ReplayState` — current playback config: ticker, date range, speed, cursor position
//! - `PlaybackStatus` — stopped, playing, or paused
//! - `SpeedPreset` — 0.25x through 100x with custom option
//! - `ReplayData` — time-indexed trades and depth for playback traversal

pub mod state;

// Re-export commonly used types
pub use state::{PlaybackStatus, ReplayData, ReplayDataStats, ReplayState, SpeedPreset};
