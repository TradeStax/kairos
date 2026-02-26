//! Download schema — Databento schema wrapper

/// Databento download schema selection.
///
/// Provides a stable API boundary so callers don't depend on the
/// third-party `databento` crate directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DownloadSchema {
    Trades,
    Mbp10,
    Mbp1,
    Ohlcv1M,
    Tbbo,
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
    pub fn as_discriminant(self) -> u16 {
        self.to_databento_schema() as u16
    }

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
