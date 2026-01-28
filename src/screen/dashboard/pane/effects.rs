use data::{ChartConfig, DateRange, FuturesTicker};
use exchange::FuturesTickerInfo;

#[derive(Debug, Clone)]
pub enum Effect {
    LoadChart {
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    },
    SwitchTickersInGroup(FuturesTickerInfo),
    FocusWidget(iced::widget::Id),
    EstimateDataCost {
        ticker: FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: DateRange,
    },
    DownloadData {
        ticker: FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: DateRange,
    },
}
