mod chart_loading;
mod feed_management;
mod pane_management;
pub mod pane;
pub mod panel;
pub mod sidebar;
mod update;
mod view;

pub use sidebar::Sidebar;

use crate::{components::display::toast::Toast, window};

#[derive(thiserror::Error, Debug, Clone)]
pub enum DashboardError {
    #[error("Fetch error: {0}")]
    Fetch(String),
    #[error("Pane set error: {0}")]
    PaneSet(String),
    #[error("Unknown error: {0}")]
    Unknown(String),
}
use data::{ChartConfig, ChartData, ChartState, LoadingStatus, WindowSpec};
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
        schema: exchange::DownloadSchema,
        date_range: data::DateRange,
    },
    DownloadData {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DownloadSchema,
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
    DrawingUndo,
    DrawingRedo,
    DrawingDuplicate,
    ScrollToLatest,
    ZoomStep(f32),
    ExchangeEvent(exchange::Event),
    ReplayTrades(FuturesTickerInfo, Vec<data::Trade>),
    ReplayRebuild(FuturesTickerInfo, Vec<data::Trade>),
    ReplaySyncPane {
        pane_id: uuid::Uuid,
        trades: Vec<data::Trade>,
    },
}

pub struct Dashboard {
    pub panes: pane_grid::State<pane::State>,
    pub focus: Option<(window::Id, pane_grid::Pane)>,
    pub popout: HashMap<window::Id, (pane_grid::State<pane::State>, WindowSpec)>,
    pub charts: HashMap<uuid::Uuid, ChartState>,
    pub market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
    pub crosshair_positions: HashMap<data::LinkGroup, (u64, f32)>,
    pub data_index: std::sync::Arc<std::sync::Mutex<data::DataIndex>>,
}

impl Dashboard {
    pub fn new(
        market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
        data_index: std::sync::Arc<std::sync::Mutex<data::DataIndex>>,
    ) -> Self {
        Self {
            panes: pane_grid::State::with_configuration(Self::default_pane_config()),
            focus: None,
            charts: HashMap::new(),
            market_data_service,
            popout: HashMap::new(),
            crosshair_positions: HashMap::new(),
            data_index,
        }
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
        schema: exchange::DownloadSchema,
        date_range: data::DateRange,
    },
    DownloadData {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DownloadSchema,
        date_range: data::DateRange,
    },
    PaneClosed {
        pane_id: uuid::Uuid,
    },
    /// Drawing tool was auto-changed (e.g. after completing a drawing)
    DrawingToolChanged(data::DrawingTool),
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
        market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
        data_index: std::sync::Arc<std::sync::Mutex<data::DataIndex>>,
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
            data_index,
        }
    }
}
