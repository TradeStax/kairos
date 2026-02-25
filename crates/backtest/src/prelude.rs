pub use crate::config::backtest::BacktestConfig;
pub use crate::config::risk::{PositionSizeMode, RiskConfig, SlippageModel};
pub use crate::core::metadata::{StrategyCategory, StrategyMetadata};
pub use crate::core::{BacktestStrategy, Signal, StrategyInput};
pub use crate::domain::metrics::PerformanceMetrics;
pub use crate::domain::result::BacktestResult;
pub use crate::domain::trade_record::{ExitReason, TradeRecord};
pub use crate::portfolio::equity::{EquityCurve, EquityPoint};
pub use crate::registry::{StrategyInfo, StrategyRegistry};
pub use kairos_data::{Candle, FuturesTickerInfo, Price, Side, Timestamp, Trade};
pub use kairos_study::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterValue, StudyConfig, Visibility,
};
