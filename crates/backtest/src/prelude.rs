pub use crate::config::backtest::BacktestConfig;
pub use crate::config::risk::{PositionSizeMode, RiskConfig, SlippageModel};
pub use crate::feed::provider::TradeProvider;
pub use crate::order::request::{BracketOrder, NewOrder, OrderRequest};
pub use crate::order::types::{OrderSide, OrderType, TimeInForce};
pub use crate::output::metrics::PerformanceMetrics;
pub use crate::output::result::BacktestResult;
pub use crate::output::trade_record::{ExitReason, TradeRecord};
pub use crate::portfolio::equity::{EquityCurve, EquityPoint};
pub use crate::strategy::metadata::{StrategyCategory, StrategyMetadata};
pub use crate::strategy::registry::{StrategyInfo, StrategyRegistry};
pub use crate::strategy::{
    BacktestError, OrderEvent, Strategy, StrategyContext, StudyBank, StudyRequest,
};
pub use kairos_data::{Candle, FuturesTickerInfo, Price, Side, Timestamp, Trade};
pub use kairos_study::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterValue, StudyConfig, Visibility,
};
