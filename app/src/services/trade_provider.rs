use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use data::{DateRange, FuturesTicker, Trade};

/// Adapter that bridges the backtest crate's [`backtest::TradeProvider`]
/// trait to the data crate's [`data::engine::DataEngine`].
pub struct EngineTradeProvider {
    engine: Arc<tokio::sync::Mutex<data::engine::DataEngine>>,
}

impl EngineTradeProvider {
    pub fn new(engine: Arc<tokio::sync::Mutex<data::engine::DataEngine>>) -> Self {
        Self { engine }
    }
}

impl backtest::TradeProvider for EngineTradeProvider {
    fn get_trades(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Trade>, String>> + Send + '_>> {
        let ticker = *ticker;
        let date_range = *date_range;
        let engine = self.engine.clone();
        Box::pin(async move {
            engine
                .lock()
                .await
                .get_trades(&ticker, &date_range, None)
                .await
                .map_err(|e| e.to_string())
        })
    }
}
