use crate::modals;
use crate::persistence::WindowSpec;
use crate::screen::dashboard;
use data::FeedId;
use std::collections::HashMap;

use crate::app::update::menu_bar;
use crate::window;

/// Messages for the backtest subsystem.
#[derive(Debug, Clone)]
pub enum BacktestMessage {
    /// Open the backtest launch modal/sidebar panel.
    OpenLaunchModal,
    /// Open the backtest manager modal.
    OpenManager,
    /// User interaction with the launch modal.
    LaunchModalInteraction(crate::screen::backtest::launch::Message),
    /// User interaction with the management modal.
    ManagerInteraction(crate::screen::backtest::manager::ManagerMessage),
    /// User clicked Run — triggers async engine run.
    Run {
        config: Box<backtest::BacktestConfig>,
    },
    /// Streamed progress event from the engine.
    ProgressEvent(backtest::BacktestProgressEvent),
    /// Engine completed — Box to keep Message size bounded.
    Completed {
        run_id: uuid::Uuid,
        result: Box<backtest::BacktestResult>,
    },
    /// Engine failed.
    Failed { run_id: uuid::Uuid, error: String },
    /// CSV export completed (or was cancelled).
    CsvExported(Option<Result<std::path::PathBuf, String>>),
}

#[derive(Debug, Clone)]
pub enum ChartMessage {
    LoadChartData {
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
        config: data::ChartConfig,
        ticker_info: data::FuturesTickerInfo,
    },
    ChartDataLoaded {
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
        ticker_info: data::FuturesTickerInfo,
        result: Result<data::ChartData, String>,
    },
    UpdateLoadingStatus,
}

#[derive(Debug, Clone)]
pub enum DownloadMessage {
    EstimateDataCost {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: data::DownloadSchema,
        date_range: data::DateRange,
    },
    DataCostEstimated {
        pane_id: uuid::Uuid,
        result: Result<(usize, Vec<chrono::NaiveDate>, Option<f64>), String>,
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
        sub_day_fraction: f32,
    },
    DataDownloadComplete {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        date_range: data::DateRange,
        result: Result<usize, String>,
    },
    ApiKeySetup(modals::download::ApiKeySetupMessage),
    HistoricalDownload(modals::download::HistoricalDownloadMessage),
    HistoricalDownloadCostEstimated {
        result: Result<(usize, Vec<chrono::NaiveDate>, Option<f64>), String>,
    },
    HistoricalDownloadComplete {
        ticker: data::FuturesTicker,
        date_range: data::DateRange,
        result: Result<usize, String>,
    },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum WindowMessage {
    TitleBarHover(bool),
    Drag(crate::window::Id),
    Minimize(crate::window::Id),
    ToggleMaximize(crate::window::Id),
    Close(crate::window::Id),
}

#[derive(Debug, Clone)]
pub enum Message {
    Sidebar(dashboard::sidebar::Message),
    Dashboard {
        /// If `None`, the active layout is used for the event.
        layout_id: Option<uuid::Uuid>,
        event: Box<dashboard::Message>,
    },
    ConnectionsMenu(modals::connections::ConnectionsMenuMessage),
    DataFeeds(modals::data_feeds::DataFeedsMessage),
    Chart(ChartMessage),
    Download(DownloadMessage),
    Tick(std::time::Instant),
    WindowEvent(window::Event),
    ExitRequested(HashMap<window::Id, WindowSpec>),
    GoBack,
    DataFolderRequested,
    RemoveNotification(usize),
    ToggleDialogModal(Option<crate::components::overlay::confirm_dialog::ConfirmDialog<Message>>),
    ThemeEditor(modals::theme::Message),
    ReinitializeService(crate::config::secrets::ApiProvider),
    DataIndexRebuilt(Result<data::DataIndex, String>),
    RithmicConnected {
        feed_id: FeedId,
        result: Result<(), String>,
    },
    RithmicSystemNames {
        server: data::RithmicServer,
        result: Result<Vec<String>, String>,
    },
    RithmicProductCodes {
        _feed_id: FeedId,
        result: Result<Vec<String>, String>,
    },
    /// Events from the DataEngine (connection lifecycle, market data, download progress).
    DataEvent(data::DataEvent),
    Replay(modals::replay::Message),
    ReplayEvent(crate::services::ReplayEvent),
    Backtest(BacktestMessage),
    MenuBar(menu_bar::Message),
    // Window control messages (custom title bar)
    #[allow(dead_code)]
    Window(WindowMessage),
    DataEngineReady(Result<super::init::services::DataEngineInit, String>),
    CacheManagement(modals::cache_management::CacheManagementMessage),
    AiStreamEvent(super::core::globals::AiStreamEvent),
    AiStreamComplete,
    /// Persist application state to disk with the given live window specs.
    /// Used by intermediate save paths (feed updates, downloads) that collect
    /// window positions asynchronously before writing.
    PersistState(HashMap<window::Id, WindowSpec>),
    /// No-op message used as completion signal for fire-and-forget async tasks
    /// (e.g. background disk persistence).
    Noop,
}
