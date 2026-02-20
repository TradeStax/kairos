use crate::config::{ParameterDef, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::StudyOutput;
use data::{Candle, ChartBasis, Price, Trade};

/// Core trait for all technical studies and indicators.
pub trait Study: Send + Sync {
    /// Unique identifier (e.g. "sma", "rsi", "volume_profile")
    fn id(&self) -> &str;

    /// Display name (e.g. "Simple Moving Average")
    fn name(&self) -> &str;

    /// Category for grouping in the UI
    fn category(&self) -> StudyCategory;

    /// Where this study renders on the chart
    fn placement(&self) -> StudyPlacement;

    /// Parameter definitions for the settings UI
    fn parameters(&self) -> &[ParameterDef];

    /// Current configuration snapshot
    fn config(&self) -> &StudyConfig;

    /// Update a single parameter by key
    fn set_parameter(&mut self, key: &str, value: ParameterValue) -> Result<(), StudyError>;

    /// Compute study values from input data
    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError>;

    /// Incrementally process new trades appended since last compute.
    /// Default implementation falls back to full recompute.
    fn append_trades(
        &mut self,
        _new_trades: &[data::Trade],
        input: &StudyInput,
    ) -> Result<(), StudyError> {
        self.compute(input)
    }

    /// Get computed output for rendering
    fn output(&self) -> &StudyOutput;

    /// Reset all computed data
    fn reset(&mut self);

    /// Clone this study into a new boxed instance
    fn clone_study(&self) -> Box<dyn Study>;
}

/// Study category for grouping in menus and search.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StudyCategory {
    Trend,
    Momentum,
    Volume,
    Volatility,
    OrderFlow,
    #[default]
    Custom,
}

impl std::fmt::Display for StudyCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StudyCategory::Trend => write!(f, "Trend"),
            StudyCategory::Momentum => write!(f, "Momentum"),
            StudyCategory::Volume => write!(f, "Volume"),
            StudyCategory::Volatility => write!(f, "Volatility"),
            StudyCategory::OrderFlow => write!(f, "Order Flow"),
            StudyCategory::Custom => write!(f, "Custom"),
        }
    }
}

/// Where a study renders relative to the price chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StudyPlacement {
    /// Drawn on the price chart (SMA, Bollinger, VWAP)
    Overlay,
    /// Separate panel below chart (RSI, MACD, Volume)
    Panel,
    /// Behind candles (Volume Profile, Value Area)
    Background,
}

impl std::fmt::Display for StudyPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StudyPlacement::Overlay => write!(f, "Overlay"),
            StudyPlacement::Panel => write!(f, "Panel"),
            StudyPlacement::Background => write!(f, "Background"),
        }
    }
}

/// Input data provided to studies for computation.
pub struct StudyInput<'a> {
    /// OHLCV candle data
    pub candles: &'a [Candle],
    /// Optional raw trades (for order flow studies)
    pub trades: Option<&'a [Trade]>,
    /// Chart basis (time or tick)
    pub basis: ChartBasis,
    /// Tick size for the instrument
    pub tick_size: Price,
    /// Optional visible range for range-limited studies
    pub visible_range: Option<(u64, u64)>,
}
