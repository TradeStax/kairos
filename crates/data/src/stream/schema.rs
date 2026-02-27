//! Download schema — stable API wrapper around Databento schema types.
//!
//! Provides a crate-local enum so callers don't depend on the third-party
//! `databento` crate directly. Feature-gated conversion methods bridge to
//! the underlying `databento::dbn::Schema` when the `databento` feature is enabled.

/// Databento download schema selection.
///
/// Maps to `databento::dbn::Schema` variants when the `databento` feature
/// is enabled, but can be used without that dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DownloadSchema {
    /// Tick-by-tick trades
    Trades,
    /// Market-by-price, 10 levels
    Mbp10,
    /// Market-by-price, 1 level (BBO)
    Mbp1,
    /// OHLCV candles, 1-minute
    Ohlcv1M,
    /// Top-of-book BBO with trades
    Tbbo,
    /// Market-by-order (full order book)
    Mbo,
}

impl std::fmt::Display for DownloadSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trades => write!(f, "Trades"),
            Self::Mbp10 => write!(f, "MBP-10"),
            Self::Mbp1 => write!(f, "MBP-1"),
            Self::Ohlcv1M => write!(f, "OHLCV-1M"),
            Self::Tbbo => write!(f, "TBBO"),
            Self::Mbo => write!(f, "MBO"),
        }
    }
}

#[cfg(feature = "databento")]
impl DownloadSchema {
    /// Returns the numeric discriminant matching the Databento schema enum
    #[must_use]
    pub fn as_discriminant(self) -> u16 {
        self.to_databento_schema() as u16
    }

    /// Converts to the corresponding `databento::dbn::Schema` variant
    #[must_use]
    pub fn to_databento_schema(self) -> databento::dbn::Schema {
        match self {
            Self::Trades => databento::dbn::Schema::Trades,
            Self::Mbp10 => databento::dbn::Schema::Mbp10,
            Self::Mbp1 => databento::dbn::Schema::Mbp1,
            Self::Ohlcv1M => databento::dbn::Schema::Ohlcv1M,
            Self::Tbbo => databento::dbn::Schema::Tbbo,
            Self::Mbo => databento::dbn::Schema::Mbo,
        }
    }
}
