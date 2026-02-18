//! Naked Point of Control (nPOC) Study
//!
//! Naked POCs are Points of Control that haven't been revisited by price
//! since they were formed. They often act as support/resistance levels.

/// nPOC configuration
#[derive(Debug, Clone, Copy)]
pub struct NpocConfig {
    /// Number of candles to look ahead for revisitation
    pub lookback: usize,
    /// Line height in pixels
    pub line_height: f32,
    /// Line alpha
    pub alpha: f32,
}

impl Default for NpocConfig {
    fn default() -> Self {
        Self {
            lookback: 100,
            line_height: 2.0,
            alpha: 0.5,
        }
    }
}
