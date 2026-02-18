mod services;
mod state;
mod subscriptions;
mod update;

use crate::layout::{LayoutId, configuration};
use crate::modal::{LayoutManager, ThemeEditor, audio::AudioStream};
use crate::modal::{dashboard_modal, main_dialog_modal};
use crate::screen::dashboard::{
    self, Dashboard,
    tickers_table::{self, TickersTable},
};
use crate::style::tokens;
use crate::component;
use crate::component::display::tooltip::tooltip;
use crate::widget::toast::{self, Toast};
use crate::{split_column, style, window};
use data::config::theme::default_theme;
use data::{layout::WindowSpec, sidebar};

use data::FeedId;
use iced::{
    Alignment, Element, Subscription, Task, padding,
    widget::{
        button, column, container, pane_grid, pick_list, row, rule, scrollable, text,
        tooltip::Position as TooltipPosition,
    },
};
use std::{borrow::Cow, collections::HashMap, sync::OnceLock, vec};

// Global download progress state (shared between async tasks and subscriptions)
#[allow(clippy::type_complexity)]
static DOWNLOAD_PROGRESS: OnceLock<
    std::sync::Arc<std::sync::Mutex<HashMap<uuid::Uuid, (usize, usize)>>>,
> = OnceLock::new();

pub fn get_download_progress()
-> &'static std::sync::Arc<std::sync::Mutex<HashMap<uuid::Uuid, (usize, usize)>>> {
    DOWNLOAD_PROGRESS.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())))
}

// Global staging for Rithmic streaming events
static RITHMIC_EVENTS: OnceLock<std::sync::Arc<std::sync::Mutex<Vec<exchange::Event>>>> =
    OnceLock::new();

pub fn get_rithmic_events() -> &'static std::sync::Arc<std::sync::Mutex<Vec<exchange::Event>>> {
    RITHMIC_EVENTS.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(Vec::new())))
}

// Staging slot for Rithmic service result (non-Clone, consumed once)
static RITHMIC_SERVICE_RESULT: OnceLock<
    std::sync::Arc<std::sync::Mutex<Option<services::RithmicServiceResult>>>,
> = OnceLock::new();

pub fn get_rithmic_service_staging()
-> &'static std::sync::Arc<std::sync::Mutex<Option<services::RithmicServiceResult>>> {
    RITHMIC_SERVICE_RESULT.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None)))
}

pub struct Flowsurface {
    pub(crate) main_window: window::Window,
    pub(crate) sidebar: dashboard::Sidebar,
    pub(crate) tickers_table: TickersTable,
    pub(crate) layout_manager: LayoutManager,
    pub(crate) theme_editor: ThemeEditor,
    pub(crate) audio_stream: AudioStream,
    pub(crate) data_management_panel: crate::modal::pane::download::DataManagementPanel,
    pub(crate) connections_menu: crate::modal::pane::connections::ConnectionsMenu,
    pub(crate) data_feeds_modal: crate::modal::pane::data_feeds::DataFeedsModal,
    pub(crate) historical_download_modal:
        Option<crate::modal::pane::download::HistoricalDownloadModal>,
    pub(crate) historical_download_id: Option<uuid::Uuid>,
    pub(crate) data_feed_manager: std::sync::Arc<std::sync::Mutex<data::DataFeedManager>>,
    pub(crate) confirm_dialog: Option<crate::screen::ConfirmDialog<Message>>,
    // Service layer (optional - None when API key not configured)
    pub(crate) market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
    pub(crate) options_service: Option<std::sync::Arc<data::services::OptionsDataService>>,
    pub(crate) replay_engine:
        Option<std::sync::Arc<std::sync::Mutex<data::services::ReplayEngine>>>,
    // Rithmic connection state
    pub(crate) rithmic_client: Option<std::sync::Arc<tokio::sync::Mutex<exchange::RithmicClient>>>,
    pub(crate) rithmic_trade_repo: Option<std::sync::Arc<exchange::RithmicTradeRepository>>,
    pub(crate) rithmic_depth_repo: Option<std::sync::Arc<exchange::RithmicDepthRepository>>,
    pub(crate) rithmic_feed_id: Option<FeedId>,
    // User preferences
    pub(crate) ui_scale_factor: data::ScaleFactor,
    pub(crate) timezone: data::UserTimezone,
    pub(crate) theme: data::Theme,
    pub(crate) notifications: Vec<Toast>,
    pub(crate) downloaded_tickers:
        std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
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
    ReplayEvent(data::services::ReplayEvent),
    UpdateLoadingStatus,
}

#[derive(Debug, Clone)]
pub enum OptionsMessage {
    LoadOptionChain {
        pane_id: uuid::Uuid,
        underlying_ticker: String,
        date: chrono::NaiveDate,
    },
    OptionChainLoaded {
        pane_id: uuid::Uuid,
        result: Result<data::domain::OptionChain, String>,
    },
    #[allow(dead_code)]
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
        schema: exchange::DatabentoSchema,
        date_range: data::DateRange,
    },
    DataCostEstimated {
        pane_id: uuid::Uuid,
        #[allow(clippy::type_complexity)]
        result: Result<(usize, usize, usize, String, f64, Vec<chrono::NaiveDate>), String>,
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
        ticker: data::FuturesTicker,
        date_range: data::DateRange,
        result: Result<usize, String>,
    },
    HistoricalDownload(crate::modal::pane::download::HistoricalDownloadMessage),
    HistoricalDownloadCostEstimated {
        result: Result<(usize, usize, usize, String, f64, Vec<chrono::NaiveDate>), String>,
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
    TickersTable(tickers_table::Message),
    Dashboard {
        /// If `None`, the active layout is used for the event.
        layout_id: Option<uuid::Uuid>,
        event: dashboard::Message,
    },
    DataManagement(crate::modal::pane::download::DataManagementMessage),
    ConnectionsMenu(crate::modal::pane::connections::ConnectionsMenuMessage),
    DataFeeds(crate::modal::pane::data_feeds::DataFeedsMessage),
    DataFeedPreviewLoaded {
        feed_id: data::FeedId,
        result: Result<crate::modal::pane::data_feeds::PreviewData, String>,
    },
    Chart(ChartMessage),
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
    ToggleDialogModal(Option<crate::screen::ConfirmDialog<Message>>),
    ThemeEditor(crate::modal::theme_editor::Message),
    Layouts(crate::modal::layout_manager::Message),
    AudioStream(crate::modal::audio::Message),
    ReinitializeService(data::ApiProvider),
    RithmicConnected {
        feed_id: FeedId,
        result: Result<(), String>,
    },
    RithmicStreamEvent(exchange::Event),
}

impl Flowsurface {
    pub fn new() -> (Self, Task<Message>) {
        // Initialize services
        let market_data_result = services::initialize_market_data_service();
        let market_data_service = market_data_result.as_ref().map(|r| r.service.clone());
        let replay_engine = services::create_replay_engine(market_data_result.as_ref());
        let (options_service, _gex_service) = services::initialize_options_services();

        // Load saved state first to get persisted registry
        let saved_state_temp =
            crate::layout::load_saved_state_without_registry(market_data_service.clone());

        // Create THE SINGLE shared Arc<Mutex<>> with loaded registry data
        let downloaded_tickers = std::sync::Arc::new(std::sync::Mutex::new(
            saved_state_temp.downloaded_tickers.clone(),
        ));

        // Re-create layout manager with the shared Arc
        let layout_manager = crate::modal::LayoutManager::new(
            market_data_service.clone(),
            downloaded_tickers.clone(),
            saved_state_temp.sidebar.date_range_preset,
        );

        // Create shared data feed manager
        let data_feed_manager =
            std::sync::Arc::new(std::sync::Mutex::new(saved_state_temp.data_feeds.clone()));

        // Create final SavedState with shared Arc in layout_manager
        let saved_state = crate::layout::SavedState {
            theme: saved_state_temp.theme,
            custom_theme: saved_state_temp.custom_theme,
            layout_manager,
            main_window: saved_state_temp.main_window,
            timezone: saved_state_temp.timezone,
            sidebar: saved_state_temp.sidebar,
            scale_factor: saved_state_temp.scale_factor,
            audio_cfg: saved_state_temp.audio_cfg,
            downloaded_tickers: saved_state_temp.downloaded_tickers,
            data_feeds: saved_state_temp.data_feeds,
        };

        let (main_window_id, open_main_window) = {
            let (position, size) = saved_state.window();
            let config = window::Settings {
                size,
                position,
                exit_on_close_request: false,
                ..window::settings()
            };
            window::open(config)
        };

        // Create tickers table at app level (shared by pane dropdowns)
        let (mut tickers_table, _initial_fetch) = TickersTable::new();

        // Ticker list starts empty - tickers only appear after the user
        // connects to a data feed via the connections menu.
        log::info!("Ticker list empty until a data feed is connected");

        let sidebar = dashboard::Sidebar::new(&saved_state);

        let mut state = Self {
            main_window: window::Window::new(main_window_id),
            layout_manager: saved_state.layout_manager,
            theme_editor: ThemeEditor::new(saved_state.custom_theme),
            audio_stream: AudioStream::new(saved_state.audio_cfg),
            data_management_panel: crate::modal::pane::download::DataManagementPanel::new(),
            connections_menu: crate::modal::pane::connections::ConnectionsMenu::new(),
            data_feeds_modal: crate::modal::pane::data_feeds::DataFeedsModal::new(),
            historical_download_modal: None,
            historical_download_id: None,
            data_feed_manager,
            sidebar,
            tickers_table,
            confirm_dialog: None,
            rithmic_client: None,
            rithmic_trade_repo: None,
            rithmic_depth_repo: None,
            rithmic_feed_id: None,
            market_data_service,
            options_service,
            replay_engine,
            timezone: saved_state.timezone,
            ui_scale_factor: saved_state.scale_factor,
            theme: saved_state.theme,
            notifications: vec![],
            downloaded_tickers: downloaded_tickers.clone(),
        };

        let active_layout_id = state.layout_manager.active_layout_id().unwrap_or(
            &state
                .layout_manager
                .layouts
                .first()
                .expect("No layouts available")
                .id,
        );
        let load_layout = state.load_layout(active_layout_id.unique, main_window_id);

        // Auto-connect feeds that have auto_connect enabled
        {
            let mut feed_manager = state
                .data_feed_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let secrets = data::SecretsManager::new();

            let auto_connect_ids: Vec<data::FeedId> = feed_manager
                .feeds()
                .iter()
                .filter(|f| f.auto_connect && f.enabled)
                .map(|f| f.id)
                .collect();

            for fid in &auto_connect_ids {
                if let Some(feed) = feed_manager.get(*fid) {
                    let has_key = match feed.provider {
                        data::FeedProvider::Databento => {
                            secrets.has_api_key(data::ApiProvider::Databento)
                        }
                        data::FeedProvider::Rithmic => {
                            secrets.has_api_key(data::ApiProvider::Rithmic)
                        }
                    };
                    if has_key {
                        feed_manager.set_status(*fid, data::FeedStatus::Connected);
                        log::info!("Auto-connected feed {} on startup", fid);
                    }
                }
            }

            // Populate ticker list for auto-connected feeds
            if !auto_connect_ids.is_empty() {
                let ticker_symbols: std::collections::HashSet<String> = state
                    .downloaded_tickers
                    .lock()
                    .unwrap()
                    .list_tickers()
                    .into_iter()
                    .collect();
                if !ticker_symbols.is_empty() {
                    state.tickers_table.set_cached_filter(ticker_symbols);
                }
            }

            state.data_feeds_modal.sync_snapshot(&feed_manager);
            state.connections_menu.sync_snapshot(&feed_manager);
        }

        (
            state,
            open_main_window
                .discard()
                .chain(load_layout),
        )
    }

    pub fn title(&self, _window: window::Id) -> String {
        if let Some(id) = self.layout_manager.active_layout_id() {
            format!("XX & Company [{}]", id.name)
        } else {
            "XX & Company".to_string()
        }
    }

    pub fn theme(&self, _window: window::Id) -> iced_core::Theme {
        self.theme.0.clone()
    }

    pub fn scale_factor(&self, _window: window::Id) -> f32 {
        self.ui_scale_factor.into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        subscriptions::build_subscription(&self.tickers_table)
    }

    pub fn view(&self, id: window::Id) -> Element<'_, Message> {
        let dashboard = self.active_dashboard();
        let sidebar_pos = self.sidebar.position();

        let tickers_table = &self.tickers_table;

        let content = if id == self.main_window.id {
            let sidebar_view = self
                .sidebar
                .view(self.audio_stream.volume())
                .map(Message::Sidebar);

            let dashboard_view = dashboard
                .view(&self.main_window, tickers_table, self.timezone)
                .map(move |msg| Message::Dashboard {
                    layout_id: None,
                    event: msg,
                });

            let header_title = {
                #[cfg(target_os = "macos")]
                {
                    iced::widget::center(
                        text("XXNCO")
                            .font(iced::Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            })
                            .size(tokens::text::HEADING)
                            .style(style::title_text),
                    )
                    .height(20)
                    .align_y(Alignment::Center)
                    .padding(padding::top(tokens::spacing::XS))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    column![]
                }
            };

            let base = column![
                header_title,
                match sidebar_pos {
                    sidebar::Position::Left => row![sidebar_view, dashboard_view,],
                    sidebar::Position::Right => row![dashboard_view, sidebar_view],
                }
                .spacing(tokens::spacing::XS)
                .padding(tokens::spacing::MD),
            ];

            if let Some(menu) = self.sidebar.active_menu() {
                self.view_with_modal(base.into(), dashboard, menu)
            } else {
                base.into()
            }
        } else {
            container(
                dashboard
                    .view_window(id, &self.main_window, tickers_table, self.timezone)
                    .map(move |msg| Message::Dashboard {
                        layout_id: None,
                        event: msg,
                    }),
            )
            .padding(padding::top(style::TITLE_PADDING_TOP))
            .into()
        };

        toast::Manager::new(
            content,
            &self.notifications,
            match sidebar_pos {
                sidebar::Position::Left => Alignment::Start,
                sidebar::Position::Right => Alignment::End,
            },
            Message::RemoveNotification,
        )
        .into()
    }

    fn view_with_modal<'a>(
        &'a self,
        base: Element<'a, Message>,
        dashboard: &'a Dashboard,
        menu: sidebar::Menu,
    ) -> Element<'a, Message> {
        let sidebar_pos = self.sidebar.position();

        match menu {
            sidebar::Menu::Settings => {
                let settings_modal = {
                    let theme_picklist = {
                        let mut themes: Vec<iced::Theme> = iced_core::Theme::ALL.to_vec();

                        let default_theme = iced_core::Theme::Custom(default_theme().into());
                        themes.push(default_theme);

                        if let Some(custom_theme) = &self.theme_editor.custom_theme {
                            themes.push(custom_theme.clone());
                        }

                        pick_list(themes, Some(self.theme.0.clone()), |theme| {
                            Message::ThemeSelected(data::Theme(theme))
                        })
                    };

                    let toggle_theme_editor = button(text("Theme editor")).on_press(
                        Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(Some(
                            sidebar::Menu::ThemeEditor,
                        ))),
                    );

                    let timezone_picklist = pick_list(
                        [data::UserTimezone::Utc, data::UserTimezone::Local],
                        Some(self.timezone),
                        Message::SetTimezone,
                    );

                    let date_range_picker = pick_list(
                        sidebar::DateRangePreset::ALL,
                        Some(self.sidebar.date_range_preset()),
                        |preset| {
                            Message::Sidebar(dashboard::sidebar::Message::SetDateRangePreset(
                                preset,
                            ))
                        },
                    );

                    let scale_factor = {
                        let current_value: f32 = self.ui_scale_factor.into();

                        let decrease_btn = if current_value > data::config::MIN_SCALE {
                            button(text("-"))
                                .on_press(Message::ScaleFactorChanged((current_value - 0.1).into()))
                        } else {
                            button(text("-"))
                        };

                        let increase_btn = if current_value < data::config::MAX_SCALE {
                            button(text("+"))
                                .on_press(Message::ScaleFactorChanged((current_value + 0.1).into()))
                        } else {
                            button(text("+"))
                        };

                        container(
                            row![
                                decrease_btn,
                                text(format!("{:.0}%", current_value * 100.0))
                                    .size(tokens::text::TITLE),
                                increase_btn,
                            ]
                            .align_y(Alignment::Center)
                            .spacing(tokens::spacing::MD)
                            .padding(tokens::spacing::XS),
                        )
                        .style(style::modal_container)
                    };

                    let open_data_folder = {
                        let button =
                            button(text("Open data folder")).on_press(Message::DataFolderRequested);

                        tooltip(
                            button,
                            Some("Open the folder where the data & config is stored"),
                            TooltipPosition::Top,
                        )
                    };

                    let column_content = split_column![
                        column![open_data_folder,].spacing(tokens::spacing::MD),
                        column![text("Date range").size(tokens::text::TITLE), date_range_picker,].spacing(tokens::spacing::LG),
                        column![text("Time zone").size(tokens::text::TITLE), timezone_picklist,].spacing(tokens::spacing::LG),
                        column![text("Theme").size(tokens::text::TITLE), theme_picklist,].spacing(tokens::spacing::LG),
                        column![text("Interface scale").size(tokens::text::TITLE), scale_factor,].spacing(tokens::spacing::LG),
                        column![
                            text("Experimental").size(tokens::text::TITLE),
                            toggle_theme_editor,
                        ]
                        .spacing(tokens::spacing::LG),
                        ; spacing = tokens::spacing::XL, align_x = Alignment::Start
                    ];

                    let content = scrollable::Scrollable::with_direction(
                        column_content,
                        scrollable::Direction::Vertical(
                            scrollable::Scrollbar::new().width(8).scroller_width(6),
                        ),
                    );

                    container(content)
                        .align_x(Alignment::Start)
                        .max_width(240)
                        .padding(tokens::spacing::XXL)
                        .style(style::dashboard_modal)
                };

                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).bottom(4)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).bottom(4)),
                };

                let base_content = dashboard_modal(
                    base,
                    settings_modal,
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                );

                if let Some(dialog) = &self.confirm_dialog {
                    let on_cancel = Message::ToggleDialogModal(None);
                    let mut builder =
                        component::overlay::confirm_dialog::ConfirmDialogBuilder::new(
                            dialog.message.clone(),
                            *dialog.on_confirm.clone(),
                            on_cancel,
                        );
                    if let Some(text) = &dialog.on_confirm_btn_text {
                        builder = builder.confirm_text(text.clone());
                    }
                    builder.view(base_content)
                } else {
                    base_content
                }
            }
            sidebar::Menu::Layout => {
                let main_window = self.main_window.id;

                let manage_pane = if let Some((window_id, pane_id)) = dashboard.focus {
                    let selected_pane_str =
                        if let Some(state) = dashboard.get_pane(main_window, window_id, pane_id) {
                            let link_group_name: String =
                                state.link_group.as_ref().map_or_else(String::new, |g| {
                                    " - Group ".to_string() + &g.to_string()
                                });

                            state.content.to_string() + &link_group_name
                        } else {
                            "".to_string()
                        };

                    let is_main_window = window_id == main_window;

                    let reset_pane_button = {
                        let btn = button(text("Reset").align_x(Alignment::Center))
                            .width(iced::Length::Fill);
                        if is_main_window {
                            let dashboard_msg = Message::Dashboard {
                                layout_id: None,
                                event: dashboard::Message::Pane(
                                    main_window,
                                    dashboard::pane::Message::ReplacePane(pane_id),
                                ),
                            };

                            btn.on_press(dashboard_msg)
                        } else {
                            btn
                        }
                    };
                    let split_pane_button = {
                        let btn = button(text("Split").align_x(Alignment::Center))
                            .width(iced::Length::Fill);
                        if is_main_window {
                            let dashboard_msg = Message::Dashboard {
                                layout_id: None,
                                event: dashboard::Message::Pane(
                                    main_window,
                                    dashboard::pane::Message::SplitPane(
                                        pane_grid::Axis::Horizontal,
                                        pane_id,
                                    ),
                                ),
                            };
                            btn.on_press(dashboard_msg)
                        } else {
                            btn
                        }
                    };

                    column![
                        text(selected_pane_str),
                        row![
                            tooltip(
                                reset_pane_button,
                                if is_main_window {
                                    Some("Reset selected pane")
                                } else {
                                    None
                                },
                                TooltipPosition::Top,
                            ),
                            tooltip(
                                split_pane_button,
                                if is_main_window {
                                    Some("Split selected pane horizontally")
                                } else {
                                    None
                                },
                                TooltipPosition::Top,
                            ),
                        ]
                        .spacing(tokens::spacing::MD)
                    ]
                    .spacing(tokens::spacing::MD)
                } else {
                    column![text("No pane selected"),].spacing(tokens::spacing::MD)
                };

                let manage_layout_modal = {
                    let col = column![
                        manage_pane,
                        rule::horizontal(1.0).style(style::split_ruler),
                        self.layout_manager.view().map(Message::Layouts)
                    ];

                    container(col.align_x(Alignment::Center).spacing(20))
                        .width(260)
                        .padding(tokens::spacing::XXL)
                        .style(style::dashboard_modal)
                };

                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).top(40)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).top(40)),
                };

                dashboard_modal(
                    base,
                    manage_layout_modal,
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::Start,
                    align_x,
                )
            }
            sidebar::Menu::Connections => {
                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).bottom(80)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).bottom(80)),
                };

                dashboard_modal(
                    base,
                    self.connections_menu.view().map(Message::ConnectionsMenu),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
            sidebar::Menu::DataFeeds => {
                let data_feeds_content = self.data_feeds_modal.view().map(Message::DataFeeds);

                let mut base_content = main_dialog_modal(
                    base,
                    data_feeds_content,
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                );

                // Stack historical download modal on top if open
                if let Some(dl_modal) = &self.historical_download_modal {
                    let dl_content = dl_modal
                        .view()
                        .map(|msg| Message::Download(DownloadMessage::HistoricalDownload(msg)));
                    base_content = main_dialog_modal(
                        base_content,
                        dl_content,
                        Message::Download(DownloadMessage::HistoricalDownload(
                            crate::modal::pane::download::HistoricalDownloadMessage::Close,
                        )),
                    );
                }

                if let Some(dialog) = &self.confirm_dialog {
                    let on_cancel = Message::ToggleDialogModal(None);
                    let mut builder =
                        component::overlay::confirm_dialog::ConfirmDialogBuilder::new(
                            dialog.message.clone(),
                            *dialog.on_confirm.clone(),
                            on_cancel,
                        );
                    if let Some(text) = &dialog.on_confirm_btn_text {
                        builder = builder.confirm_text(text.clone());
                    }
                    builder.view(base_content)
                } else {
                    base_content
                }
            }
            sidebar::Menu::Audio => {
                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).top(76)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).top(76)),
                };

                // TODO: Collect active depth/L2 streams from pane states
                // so the audio modal can list subscribable instruments.
                let depth_streams_list = vec![];

                dashboard_modal(
                    base,
                    self.audio_stream
                        .view(depth_streams_list)
                        .map(Message::AudioStream),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::Start,
                    align_x,
                )
            }
            sidebar::Menu::ThemeEditor => {
                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).bottom(4)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).bottom(4)),
                };

                dashboard_modal(
                    base,
                    self.theme_editor
                        .view(&self.theme.0)
                        .map(Message::ThemeEditor),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
        }
    }
}
