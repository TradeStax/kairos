use crate::{chart, screen::dashboard::ladder};
use data::{ChartConfig, FuturesTickerInfo};

pub enum TickAction {
    Chart(chart::Action),
    Panel(ladder::Action),
    LoadChart {
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    },
}
