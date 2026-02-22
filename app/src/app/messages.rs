use crate::modals;
use crate::screen::dashboard;
use data::FeedId;
use data::state::WindowSpec;
use std::collections::HashMap;

use crate::components::chrome::menu_bar;
use crate::window;

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
    RithmicStreamEvent(exchange::Event),
    Replay(modals::replay::Message),
    ReplayEvent(data::services::ReplayEvent),
    MenuBar(menu_bar::Message),
    // Window control messages (custom title bar)
    TitleBarHover(bool),
    WindowDrag(window::Id),
    WindowMinimize(window::Id),
    WindowToggleMaximize(window::Id),
    WindowClose(window::Id),
    ServicesReady(super::services::AllServicesResult),
}
