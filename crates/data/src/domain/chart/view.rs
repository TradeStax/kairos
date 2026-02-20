//! View configuration and loading status types

use serde::{Deserialize, Serialize};

/// Loading status for chart data
#[derive(Debug, Clone, PartialEq)]
pub enum LoadingStatus {
    /// Idle, no loading in progress
    Idle,

    /// Downloading from exchange
    Downloading {
        schema: DataSchema,
        days_total: usize,
        days_complete: usize,
        current_day: String,
    },

    /// Loading from local cache
    LoadingFromCache {
        schema: DataSchema,
        days_total: usize,
        days_loaded: usize,
        items_loaded: usize,
    },

    /// Building chart (aggregating, processing)
    Building {
        operation: String,
        progress: f32, // 0.0 to 1.0
    },

    /// Ready to display
    Ready,

    /// Error occurred
    Error { message: String },
}

impl LoadingStatus {
    pub fn is_loading(&self) -> bool {
        matches!(
            self,
            LoadingStatus::Downloading { .. }
                | LoadingStatus::LoadingFromCache { .. }
                | LoadingStatus::Building { .. }
        )
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, LoadingStatus::Ready)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, LoadingStatus::Error { .. })
    }
}

/// Data schema being loaded
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSchema {
    Trades,
    MBP10,
    OHLCV,
    Options,
}

impl std::fmt::Display for DataSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSchema::Trades => write!(f, "Trades"),
            DataSchema::MBP10 => write!(f, "MBP-10"),
            DataSchema::OHLCV => write!(f, "OHLCV"),
            DataSchema::Options => write!(f, "Options"),
        }
    }
}

/// View configuration for chart layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewConfig {
    pub splits: Vec<f32>,
    pub autoscale: Option<Autoscale>,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            splits: vec![],
            autoscale: Some(Autoscale::CenterLatest),
        }
    }
}

/// Autoscale mode for charts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Autoscale {
    CenterLatest,
    FitAll,
    Disabled,
}
