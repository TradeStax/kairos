pub(crate) mod grid;
pub mod pane;
pub mod ladder;
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
use crate::persistence::WindowSpec;
use data::{ChartConfig, ChartData, FuturesTickerInfo, LoadingStatus};

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
        ticker_info: FuturesTickerInfo,
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
        schema: data::DownloadSchema,
        date_range: data::DateRange,
    },
    DownloadData {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: data::DownloadSchema,
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
    DrawingToolSelected(crate::drawing::DrawingTool),
    DrawingSnapToggled,
    DrawingUndo,
    DrawingRedo,
    DrawingDuplicate,
    ScrollToLatest,
    ZoomStep(f32),
    LiveData(data::DataEvent),
    ReplayTrades(FuturesTickerInfo, Vec<data::Trade>),
    ReplayRebuild(FuturesTickerInfo, Vec<data::Trade>),
    ReplaySyncPane {
        pane_id: uuid::Uuid,
        trades: Vec<data::Trade>,
    },
}

pub struct Dashboard {
    pub(crate) panes: pane_grid::State<pane::State>,
    pub(crate) focus: Option<(window::Id, pane_grid::Pane)>,
    pub(crate) popout: HashMap<window::Id, (pane_grid::State<pane::State>, WindowSpec)>,
    pub(crate) crosshair_positions:
        HashMap<crate::screen::dashboard::pane::config::LinkGroup, (u64, f32)>,
    pub(crate) data_index: std::sync::Arc<std::sync::Mutex<data::DataIndex>>,
}

impl Dashboard {
    pub fn new(data_index: std::sync::Arc<std::sync::Mutex<data::DataIndex>>) -> Self {
        Self {
            panes: pane_grid::State::with_configuration(Self::default_pane_config()),
            focus: None,
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
        schema: data::DownloadSchema,
        date_range: data::DateRange,
    },
    DownloadData {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: data::DownloadSchema,
        date_range: data::DateRange,
    },
    PaneClosed {
        pane_id: uuid::Uuid,
    },
    /// Drawing tool was auto-changed (e.g. after completing a drawing)
    DrawingToolChanged(crate::drawing::DrawingTool),
    /// AI assistant wants to send a message
    AiRequest {
        pane_id: uuid::Uuid,
        user_message: String,
    },
    /// AI pane credential modal: save an OpenRouter API key
    SaveAiApiKey(String),
    /// AI context query from a chart drawing selection
    AiContextQuery {
        source_pane_id: uuid::Uuid,
        context: String,
        question: String,
    },
    /// AI preferences changed (persist to saved state)
    AiPreferencesChanged {
        model: String,
        temperature: f32,
        max_tokens: u32,
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
        _market_data_service: Option<()>,
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
            popout,
            crosshair_positions: HashMap::new(),
            data_index,
        }
    }
}
