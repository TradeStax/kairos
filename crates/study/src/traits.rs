use crate::config::{ParameterDef, ParameterTab, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{CandleRenderConfig, StudyOutput};
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

    /// Mutable access to configuration for the default
    /// `set_parameter` implementation.
    fn config_mut(&mut self) -> &mut StudyConfig;

    /// Update a single parameter by key.
    ///
    /// Default implementation validates against `parameters()` definitions
    /// and sets the value. Override only if custom cross-field validation
    /// is needed.
    fn set_parameter(
        &mut self,
        key: &str,
        value: ParameterValue,
    ) -> Result<(), StudyError> {
        // Borrow parameters slice before mutable borrow of config
        let params = self.parameters();
        let def = params
            .iter()
            .find(|p| p.key == key)
            .ok_or_else(|| StudyError::InvalidParameter {
                key: key.to_string(),
                reason: "unknown parameter".to_string(),
            })?;

        def.validate(&value).map_err(|reason| {
            StudyError::InvalidParameter {
                key: key.to_string(),
                reason,
            }
        })?;

        self.config_mut().set(key, value);
        Ok(())
    }

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

    /// Optional render configuration for CandleReplace studies.
    /// Returns layout constants that override the chart's default
    /// cell sizing, zoom bounds, and initial candle window.
    fn candle_render_config(&self) -> Option<CandleRenderConfig> {
        None
    }

    /// Optional custom tab labels for the settings UI.
    /// Returns (label, tab) pairs. When None, default tab names are used.
    fn tab_labels(&self) -> Option<&[(&'static str, ParameterTab)]> {
        None
    }

    /// Clone this study into a new boxed instance
    fn clone_study(&self) -> Box<dyn Study>;
}

/// Study category for grouping in menus and search.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash,
    serde::Serialize, serde::Deserialize,
)]
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    serde::Serialize, serde::Deserialize,
)]
pub enum StudyPlacement {
    /// Drawn on the price chart (SMA, Bollinger, VWAP)
    Overlay,
    /// Separate panel below chart (RSI, MACD, Volume)
    Panel,
    /// Behind candles (Volume Profile, Value Area)
    Background,
    /// Replaces standard candle rendering entirely.
    /// Only one CandleReplace study can be active at a time.
    CandleReplace,
}

impl std::fmt::Display for StudyPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StudyPlacement::Overlay => write!(f, "Overlay"),
            StudyPlacement::Panel => write!(f, "Panel"),
            StudyPlacement::Background => write!(f, "Background"),
            StudyPlacement::CandleReplace => write!(f, "Candle Replace"),
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
