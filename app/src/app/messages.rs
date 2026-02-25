use crate::modals;
use crate::screen::dashboard;
use data::FeedId;
use data::state::WindowSpec;
use std::collections::HashMap;

use crate::app::update::menu_bar;
use crate::infra::window;

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
    ManagerInteraction(
        crate::screen::backtest::manager::ManagerMessage,
    ),
    /// User clicked Run — triggers async engine run.
    Run { config: backtest::BacktestConfig },
    /// Streamed progress event from the engine.
    ProgressEvent(backtest::BacktestProgressEvent),
    /// Engine completed — Box to keep Message size bounded.
    Completed {
        run_id: uuid::Uuid,
        result: Box<backtest::BacktestResult>,
    },
    /// Engine failed.
    Failed {
        run_id: uuid::Uuid,
        error: String,
    },
    /// CSV export completed (or was cancelled).
    CsvExported(Option<Result<std::path::PathBuf, String>>),
}

#[derive(Debug, Clone)]
pub enum ChartMessage {
    LoadChartData {
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
        config: data::ChartConfig,
        ticker_info: exchange::FuturesTickerInfo,
    },
    ChartDataLoaded {
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
        result: Result<data::ChartData, String>,
    },
    UpdateLoadingStatus,
    LoadingStatusesReady(std::collections::HashMap<String, data::LoadingStatus>),
}

#[cfg(feature = "options")]
#[derive(Debug, Clone)]
pub enum OptionsMessage {
    OptionChainLoaded {
        pane_id: uuid::Uuid,
        result: Result<data::domain::OptionChain, String>,
    },
    GexProfileLoaded {
        pane_id: uuid::Uuid,
        result: Result<data::domain::GexProfile, String>,
    },
}

#[derive(Debug, Clone)]
pub enum DownloadMessage {
    EstimateDataCost {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DownloadSchema,
        date_range: data::DateRange,
    },
    DataCostEstimated {
        pane_id: uuid::Uuid,
        result: Result<data::DataRequestEstimate, String>,
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
        ticker: data::FuturesTicker,
        date_range: data::DateRange,
        result: Result<usize, String>,
    },
    ApiKeySetup(modals::download::ApiKeySetupMessage),
    HistoricalDownload(modals::download::HistoricalDownloadMessage),
    HistoricalDownloadCostEstimated {
        result: Result<data::DataRequestEstimate, String>,
    },
    HistoricalDownloadComplete {
        ticker: data::FuturesTicker,
        date_range: data::DateRange,
        result: Result<usize, String>,
    },
}

#[derive(Debug, Clone)]
pub enum WindowMessage {
    TitleBarHover(bool),
    Drag(crate::infra::window::Id),
    Minimize(crate::infra::window::Id),
    ToggleMaximize(crate::infra::window::Id),
    Close(crate::infra::window::Id),
}

#[derive(Debug, Clone)]
pub enum Message {
    Sidebar(dashboard::sidebar::Message),
    Dashboard {
        /// If `None`, the active layout is used for the event.
        layout_id: Option<uuid::Uuid>,
        event: dashboard::Message,
    },
    ConnectionsMenu(modals::connections::ConnectionsMenuMessage),
    DataFeeds(modals::data_feeds::DataFeedsMessage),
    DataFeedPreviewLoaded {
        feed_id: data::FeedId,
        result: Result<modals::data_feeds::PreviewData, String>,
    },
    Chart(ChartMessage),
    #[cfg(feature = "options")]
    Options(OptionsMessage),
    Download(DownloadMessage),
    Tick(std::time::Instant),
    WindowEvent(window::Event),
    ExitRequested(HashMap<window::Id, WindowSpec>),
    GoBack,
    DataFolderRequested,
    ThemeSelected(data::Theme),
    ScaleFactorChanged(data::ScaleFactor),
    SetTimezone(data::UserTimezone),
    RemoveNotification(usize),
    ToggleDialogModal(
        Option<crate::components::overlay::confirm_dialog::ConfirmDialog<Message>>,
    ),
    ThemeEditor(modals::theme::Message),
    Layouts(modals::layout::Message),
    ReinitializeService(data::config::secrets::ApiProvider),
    DataIndexRebuilt(Result<data::DataIndex, String>),
    RithmicConnected {
        feed_id: FeedId,
        result: Result<(), String>,
    },
    RithmicSystemNames {
        server: data::feed::RithmicServer,
        result: Result<Vec<String>, String>,
    },
    RithmicProductCodes {
        feed_id: FeedId,
        result: Result<Vec<String>, String>,
    },
    RithmicStreamEvent(exchange::Event),
    Replay(modals::replay::Message),
    ReplayEvent(data::services::ReplayEvent),
    Backtest(BacktestMessage),
    MenuBar(menu_bar::Message),
    // Window control messages (custom title bar)
    Window(WindowMessage),
    ServicesReady(super::init::services::AllServicesResult),
    AiStreamEvent(super::core::globals::AiStreamEventClone),
    AiStreamComplete,
}
