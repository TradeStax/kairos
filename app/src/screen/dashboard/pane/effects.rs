use data::{ChartConfig, DateRange, DrawingTool, FuturesTicker};
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
        schema: exchange::DownloadSchema,
        date_range: DateRange,
    },
    DownloadData {
        ticker: FuturesTicker,
        schema: exchange::DownloadSchema,
        date_range: DateRange,
    },
    /// Drawing tool was auto-changed (e.g. after completing a drawing)
    DrawingToolChanged(DrawingTool),
    /// Crosshair position changed in a linked pane
    CrosshairSync { interval: Option<u64> },
}
