use crate::chart;
use data::{ChartConfig, FuturesTickerInfo};

pub enum TickAction {
    Chart(chart::Action),
    LoadChart {
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    },
}
