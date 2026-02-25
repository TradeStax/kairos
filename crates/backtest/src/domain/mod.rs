pub mod metrics;
pub mod progress;
pub mod result;
pub mod trade_record;

pub use metrics::PerformanceMetrics;
pub use progress::BacktestProgressEvent;
pub use result::BacktestResult;
pub use trade_record::{ExitReason, TradeRecord};
