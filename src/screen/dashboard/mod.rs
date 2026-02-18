pub mod pane;
pub mod panel;
pub mod sidebar;
pub mod tickers_table;
mod chart_loading;
mod feed_management;
mod pane_management;
mod update;
mod view;

pub use sidebar::Sidebar;

use super::DashboardError;
use crate::{
    widget::toast::Toast,
    window,
};
use data::{
    ChartConfig, ChartData, ChartState, LoadingStatus,
    WindowSpec,
};
use exchange::FuturesTickerInfo;

use iced::widget::pane_grid::{self, Configuration};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Message {
    Pane(window::Id, pane::Message),
    ChangePaneStatus(uuid::Uuid, LoadingStatus),
    SavePopoutSpecs(HashMap<window::Id, WindowSpec>),
    ErrorOccurred(Option<uuid::Uuid>, DashboardError),
    Notification(Toast),
    ChartDataLoaded {
        pane_id: uuid::Uuid,
        chart_data: ChartData,
    },
    LoadChart {
        pane_id: uuid::Uuid,
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    },
    EstimateDataCost {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: data::DateRange,
    },
    DataCostEstimated {
        pane_id: uuid::Uuid,
        total_days: usize,
        cached_days: usize,
        uncached_days: usize,
        gaps_desc: String,
        actual_cost_usd: f64,
        cached_dates: Vec<chrono::NaiveDate>,
    },
    DownloadData {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: data::DateRange,
    },
    DataDownloadProgress {
        pane_id: uuid::Uuid,
        current: usize,
        total: usize,
    },
    DataDownloadComplete {
        pane_id: uuid::Uuid,
        days_downloaded: usize,
    },
    DrawingToolSelected(data::DrawingTool),
    DrawingSnapToggled,
    ExchangeEvent(exchange::Event),
}

pub struct Dashboard {
    pub panes: pane_grid::State<pane::State>,
    pub focus: Option<(window::Id, pane_grid::Pane)>,
    pub popout: HashMap<window::Id, (pane_grid::State<pane::State>, WindowSpec)>,
    pub charts: HashMap<uuid::Uuid, ChartState>,
    pub market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
    pub crosshair_positions: HashMap<data::LinkGroup, (u64, f32)>,
    pub downloaded_tickers: std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
    pub date_range_preset: data::sidebar::DateRangePreset,
}

impl Dashboard {
    pub fn new(
        market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
        downloaded_tickers: std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
        date_range_preset: data::sidebar::DateRangePreset,
    ) -> Self {
        Self {
            panes: pane_grid::State::with_configuration(Self::default_pane_config()),
            focus: None,
            charts: HashMap::new(),
            market_data_service,
            popout: HashMap::new(),
            crosshair_positions: HashMap::new(),
            downloaded_tickers,
            date_range_preset,
        }
    }

    pub fn set_date_range_preset(&mut self, preset: data::sidebar::DateRangePreset) {
        self.date_range_preset = preset;
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Notification(Toast),
    LoadChart {
        pane_id: uuid::Uuid,
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    },
    EstimateDataCost {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: data::DateRange,
    },
    DownloadData {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: data::DateRange,
    },
    PaneClosed {
        pane_id: uuid::Uuid,
    },
}

impl Dashboard {
    fn default_pane_config() -> Configuration<pane::State> {
        Configuration::Split {
            axis: pane_grid::Axis::Vertical,
            ratio: 0.5,
            a: Box::new(Configuration::Pane(pane::State::default())),
            b: Box::new(Configuration::Pane(pane::State::default())),
        }
    }

    pub fn from_config(
        panes: Configuration<pane::State>,
        popout_windows: Vec<(Configuration<pane::State>, WindowSpec)>,
        _layout_id: uuid::Uuid,
        market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
        downloaded_tickers: std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
        date_range_preset: data::sidebar::DateRangePreset,
    ) -> Self {
        let panes = pane_grid::State::with_configuration(panes);

        let mut popout = HashMap::new();

        for (pane, specs) in popout_windows {
            popout.insert(
                window::Id::unique(),
                (pane_grid::State::with_configuration(pane), specs),
            );
        }

        Self {
            panes,
            focus: None,
            charts: HashMap::new(),
            market_data_service,
            popout,
            crosshair_positions: HashMap::new(),
            downloaded_tickers,
            date_range_preset,
        }
    }
}
