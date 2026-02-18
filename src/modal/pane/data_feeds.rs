//! Data Feeds Modal
//!
//! Split-pane dialog for managing data feed connections. Left panel shows
//! the feed list split into "Datasets" and "Connections" sections, right
//! panel shows the edit form for realtime connections or a preview panel
//! for historical datasets.

use crate::style;
use data::{
    self,
    feed::{
        DataFeed, DataFeedManager, DatabentoFeedConfig, FeedConfig, FeedId, FeedKind, FeedProvider,
        FeedStatus, HistoricalDatasetInfo, RithmicEnvironment, RithmicFeedConfig,
    },
};
use iced::{
    Alignment, Color, Element, Length, padding,
    widget::{
        button, canvas, column, container, mouse_area, pick_list, row, rule, scrollable, space,
        stack, text, text_input,
    },
};

// ====================================================================
// Preview data for historical datasets
// ====================================================================

/// Preview data loaded for a historical dataset
#[derive(Debug, Clone)]
pub struct PreviewData {
    pub feed_id: FeedId,
    pub price_line: Vec<(u64, f64)>,
    pub trades: Vec<TradePreviewRow>,
    pub total_trades: usize,
    pub date_range_str: String,
}

#[derive(Debug, Clone)]
pub struct TradePreviewRow {
    pub time: String,
    pub price: String,
    pub size: String,
    pub side: String,
}

// ====================================================================
// Price line chart (canvas)
// ====================================================================

struct PriceLineChart {
    points: Vec<(u64, f64)>,
}

impl<Message> canvas::Program<Message> for PriceLineChart {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        if self.points.len() < 2 {
            return vec![];
        }

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let palette = theme.extended_palette();
        let line_color = palette.primary.base.color;

        let (min_t, max_t) = self
            .points
            .iter()
            .fold((u64::MAX, u64::MIN), |(lo, hi), (t, _)| {
                (lo.min(*t), hi.max(*t))
            });
        let (min_p, max_p) = self
            .points
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), (_, p)| {
                (lo.min(*p), hi.max(*p))
            });

        let t_range = (max_t - min_t).max(1) as f64;
        let p_range = (max_p - min_p).max(0.01);
        let w = bounds.width;
        let h = bounds.height;
        let pad = 4.0;

        let to_point = |t: u64, p: f64| -> iced::Point {
            let x = pad + ((t - min_t) as f64 / t_range) as f32 * (w - 2.0 * pad);
            let y = pad + (1.0 - ((p - min_p) / p_range) as f32) * (h - 2.0 * pad);
            iced::Point::new(x, y)
        };

        // Build line path
        let mut builder = canvas::path::Builder::new();
        let first = self.points[0];
        builder.move_to(to_point(first.0, first.1));
        for &(t, p) in &self.points[1..] {
            builder.line_to(to_point(t, p));
        }
        let line_path = builder.build();

        frame.stroke(
            &line_path,
            canvas::Stroke::default()
                .with_color(line_color)
                .with_width(1.5),
        );

        // Fill area under line
        let mut fill_builder = canvas::path::Builder::new();
        let first_pt = to_point(first.0, first.1);
        fill_builder.move_to(iced::Point::new(first_pt.x, h));
        fill_builder.line_to(first_pt);
        for &(t, p) in &self.points[1..] {
            fill_builder.line_to(to_point(t, p));
        }
        let last = self.points.last().unwrap();
        let last_pt = to_point(last.0, last.1);
        fill_builder.line_to(iced::Point::new(last_pt.x, h));
        fill_builder.close();

        frame.fill(
            &fill_builder.build(),
            Color {
                a: 0.1,
                ..line_color
            },
        );

        vec![frame.into_geometry()]
    }
}

// ====================================================================
// DataFeedsModal
// ====================================================================

/// Data Feeds modal state
#[derive(Debug, Clone)]
pub struct DataFeedsModal {
    selected_feed: Option<FeedId>,
    edit_form: EditForm,
    is_creating: bool,
    has_changes: bool,
    feeds_snapshot: DataFeedManager,
    /// "+" popup open
    add_popup_open: bool,
    /// Ticker multi-select dropdown open
    ticker_dropdown_open: bool,
    /// Preview data for selected historical feed
    preview_data: Option<PreviewData>,
    preview_loading: bool,
}

impl PartialEq for DataFeedsModal {
    fn eq(&self, other: &Self) -> bool {
        self.selected_feed == other.selected_feed
            && self.edit_form == other.edit_form
            && self.is_creating == other.is_creating
            && self.has_changes == other.has_changes
            && self.feeds_snapshot == other.feeds_snapshot
            && self.add_popup_open == other.add_popup_open
            && self.preview_loading == other.preview_loading
    }
}

/// Form state for editing a feed
#[derive(Debug, Clone, PartialEq)]
struct EditForm {
    provider: Option<FeedProvider>,
    name: String,
    priority: String,
    // Databento
    api_key: String,
    cache_enabled: bool,
    cache_max_days: String,
    // Rithmic
    environment: RithmicEnvironment,
    system_name: String,
    user_id: String,
    password: String,
    auto_reconnect: bool,
    subscribed_tickers: Vec<String>,
}

impl Default for EditForm {
    fn default() -> Self {
        Self {
            provider: None,
            name: String::new(),
            priority: "10".to_string(),
            api_key: String::new(),
            cache_enabled: true,
            cache_max_days: "90".to_string(),
            environment: RithmicEnvironment::Demo,
            system_name: String::new(),
            user_id: String::new(),
            password: String::new(),
            auto_reconnect: true,
            subscribed_tickers: Vec::new(),
        }
    }
}

impl EditForm {
    fn from_feed(feed: &DataFeed) -> Self {
        let mut form = Self {
            provider: Some(feed.provider),
            name: feed.name.clone(),
            priority: feed.priority.to_string(),
            ..Default::default()
        };

        match &feed.config {
            FeedConfig::Databento(cfg) => {
                form.cache_enabled = cfg.cache_enabled;
                form.cache_max_days = cfg.cache_max_days.to_string();
            }
            FeedConfig::Rithmic(cfg) => {
                form.environment = cfg.environment;
                form.system_name = cfg.system_name.clone();
                form.user_id = cfg.user_id.clone();
                form.auto_reconnect = cfg.auto_reconnect;
                form.subscribed_tickers = cfg.subscribed_tickers.clone();
            }
        }

        form
    }

    fn for_provider(provider: FeedProvider) -> Self {
        match provider {
            FeedProvider::Databento => Self {
                provider: Some(FeedProvider::Databento),
                name: "New Connection".to_string(),
                priority: "10".to_string(),
                ..Default::default()
            },
            FeedProvider::Rithmic => Self {
                provider: Some(FeedProvider::Rithmic),
                name: "New Connection".to_string(),
                priority: "5".to_string(),
                ..Default::default()
            },
        }
    }
}

/// Messages for the data feeds modal
#[derive(Debug, Clone)]
pub enum DataFeedsMessage {
    // Left panel
    SelectFeed(FeedId),
    RemoveFeed(FeedId),
    // "+" popup
    ToggleAddPopup,
    CloseAddPopup,
    AddRealtime,
    OpenHistoricalDownload,
    // Right panel - form
    SetProvider(FeedProvider),
    SetName(String),
    SetPriority(String),
    SaveFeed,
    CancelEdit,
    // Databento fields
    SetApiKey(String),
    SetCacheEnabled(bool),
    SetCacheMaxDays(String),
    // Rithmic fields
    SetEnvironment(RithmicEnvironment),
    SetSystemName(String),
    SetUserId(String),
    SetPassword(String),
    SetAutoReconnect(bool),
    ToggleTickerDropdown,
    CloseTickerDropdown,
    ToggleTicker(String),
    // Connection actions
    ConnectFeed(FeedId),
    DisconnectFeed(FeedId),
    // Status updates
    FeedStatusChanged(FeedId, FeedStatus),
    // Preview
    PreviewLoaded(FeedId, Result<PreviewData, String>),
}

/// Actions emitted by the modal to the parent
pub enum Action {
    ConnectFeed(FeedId),
    DisconnectFeed(FeedId),
    FeedsUpdated,
    OpenHistoricalDownload,
    LoadPreview(FeedId, HistoricalDatasetInfo),
}

impl DataFeedsModal {
    pub fn new() -> Self {
        Self {
            selected_feed: None,
            edit_form: EditForm::default(),
            is_creating: false,
            has_changes: false,
            feeds_snapshot: DataFeedManager::default(),
            add_popup_open: false,
            ticker_dropdown_open: false,
            preview_data: None,
            preview_loading: false,
        }
    }

    pub fn sync_snapshot(&mut self, manager: &DataFeedManager) {
        self.feeds_snapshot = manager.clone();
    }

    pub fn update(
        &mut self,
        message: DataFeedsMessage,
        feed_manager: &mut DataFeedManager,
    ) -> Option<Action> {
        match message {
            // ---- Left panel ----
            DataFeedsMessage::SelectFeed(id) => {
                if let Some(feed) = feed_manager.get(id) {
                    self.edit_form = EditForm::from_feed(feed);
                    self.selected_feed = Some(id);
                    self.is_creating = false;
                    self.has_changes = false;
                    self.ticker_dropdown_open = false;

                    // Load preview for historical feeds
                    if let Some(info) = feed.dataset_info() {
                        if self
                            .preview_data
                            .as_ref()
                            .map(|p| p.feed_id != id)
                            .unwrap_or(true)
                        {
                            self.preview_loading = true;
                            self.preview_data = None;
                            return Some(Action::LoadPreview(id, info.clone()));
                        }
                    } else {
                        self.preview_data = None;
                        self.preview_loading = false;
                    }
                }
            }
            DataFeedsMessage::RemoveFeed(id) => {
                feed_manager.remove(id);
                if self.selected_feed == Some(id) {
                    self.selected_feed = None;
                    self.is_creating = false;
                    self.has_changes = false;
                    self.preview_data = None;
                }
                return Some(Action::FeedsUpdated);
            }

            // ---- "+" popup ----
            DataFeedsMessage::ToggleAddPopup => {
                self.add_popup_open = !self.add_popup_open;
            }
            DataFeedsMessage::CloseAddPopup => {
                self.add_popup_open = false;
            }
            DataFeedsMessage::AddRealtime => {
                self.add_popup_open = false;
                let feed = DataFeed::new_rithmic("New Connection");
                let id = feed.id;
                feed_manager.add(feed);
                self.selected_feed = Some(id);
                self.is_creating = true;
                self.has_changes = false;
                self.edit_form = EditForm::for_provider(FeedProvider::Rithmic);
                return Some(Action::FeedsUpdated);
            }
            DataFeedsMessage::OpenHistoricalDownload => {
                self.add_popup_open = false;
                return Some(Action::OpenHistoricalDownload);
            }

            // ---- Right panel: form ----
            DataFeedsMessage::SetProvider(provider) => {
                if self.is_creating {
                    self.edit_form.provider = Some(provider);
                    self.edit_form.name = "New Connection".to_string();
                    match provider {
                        FeedProvider::Databento => {
                            self.edit_form.api_key = String::new();
                            self.edit_form.cache_enabled = true;
                            self.edit_form.cache_max_days = "90".to_string();
                            self.edit_form.priority = "10".to_string();
                        }
                        FeedProvider::Rithmic => {
                            self.edit_form.environment = RithmicEnvironment::Demo;
                            self.edit_form.system_name = String::new();
                            self.edit_form.user_id = String::new();
                            self.edit_form.password = String::new();
                            self.edit_form.auto_reconnect = true;
                            self.edit_form.subscribed_tickers = Vec::new();
                            self.edit_form.priority = "5".to_string();
                        }
                    }
                    if let Some(id) = self.selected_feed {
                        if let Some(feed) = feed_manager.get_mut(id) {
                            feed.provider = provider;
                            feed.name = "New Connection".to_string();
                            feed.config = match provider {
                                FeedProvider::Databento => {
                                    FeedConfig::Databento(DatabentoFeedConfig::default())
                                }
                                FeedProvider::Rithmic => {
                                    FeedConfig::Rithmic(RithmicFeedConfig::default())
                                }
                            };
                            feed.priority = match provider {
                                FeedProvider::Databento => 10,
                                FeedProvider::Rithmic => 5,
                            };
                        }
                    }
                    self.has_changes = true;
                }
            }
            DataFeedsMessage::SaveFeed => {
                if let Some(id) = self.selected_feed {
                    let provider = self.edit_form.provider;

                    if provider == Some(FeedProvider::Databento)
                        && !self.edit_form.api_key.is_empty()
                    {
                        let secrets = data::SecretsManager::new();
                        if let Err(e) = secrets
                            .set_api_key(data::ApiProvider::Databento, &self.edit_form.api_key)
                        {
                            log::warn!("Failed to save Databento API key: {}", e);
                        }
                    }
                    if provider == Some(FeedProvider::Rithmic)
                        && !self.edit_form.password.is_empty()
                    {
                        let secrets = data::SecretsManager::new();
                        if let Err(e) = secrets
                            .set_api_key(data::ApiProvider::Rithmic, &self.edit_form.password)
                        {
                            log::warn!("Failed to save Rithmic password: {}", e);
                        }
                    }

                    if let Some(feed) = feed_manager.get_mut(id) {
                        self.apply_form_to_feed(feed);
                    }

                    self.is_creating = false;
                    self.has_changes = false;
                    return Some(Action::FeedsUpdated);
                }
            }
            DataFeedsMessage::CancelEdit => {
                if self.is_creating {
                    if let Some(id) = self.selected_feed {
                        feed_manager.remove(id);
                    }
                    self.selected_feed = None;
                    self.is_creating = false;
                    self.has_changes = false;
                    return Some(Action::FeedsUpdated);
                } else {
                    if let Some(id) = self.selected_feed {
                        if let Some(feed) = feed_manager.get(id) {
                            self.edit_form = EditForm::from_feed(feed);
                            self.has_changes = false;
                        }
                    }
                }
            }

            // ---- Connection actions ----
            DataFeedsMessage::ConnectFeed(id) => {
                return Some(Action::ConnectFeed(id));
            }
            DataFeedsMessage::DisconnectFeed(id) => {
                return Some(Action::DisconnectFeed(id));
            }

            // ---- Status updates ----
            DataFeedsMessage::FeedStatusChanged(id, status) => {
                feed_manager.set_status(id, status);
            }

            // ---- Preview ----
            DataFeedsMessage::PreviewLoaded(feed_id, result) => {
                self.preview_loading = false;
                match result {
                    Ok(data) => {
                        self.preview_data = Some(data);
                    }
                    Err(e) => {
                        log::warn!("Failed to load preview for {}: {}", feed_id, e);
                        self.preview_data = None;
                    }
                }
            }

            // ---- Form field setters ----
            DataFeedsMessage::SetName(v) => {
                self.edit_form.name = v;
                self.has_changes = true;

                // For historical feeds, also update the name in the
                // manager immediately
                if let Some(id) = self.selected_feed {
                    if let Some(feed) = feed_manager.get_mut(id) {
                        if feed.is_historical() {
                            feed.name = self.edit_form.name.clone();
                            return Some(Action::FeedsUpdated);
                        }
                    }
                }
            }
            DataFeedsMessage::SetPriority(v) => {
                self.edit_form.priority = v;
                self.has_changes = true;
            }
            DataFeedsMessage::SetApiKey(v) => {
                self.edit_form.api_key = v;
                self.has_changes = true;
            }
            DataFeedsMessage::SetCacheEnabled(v) => {
                self.edit_form.cache_enabled = v;
                self.has_changes = true;
            }
            DataFeedsMessage::SetCacheMaxDays(v) => {
                self.edit_form.cache_max_days = v;
                self.has_changes = true;
            }
            DataFeedsMessage::SetEnvironment(v) => {
                self.edit_form.environment = v;
                self.has_changes = true;
            }
            DataFeedsMessage::SetSystemName(v) => {
                self.edit_form.system_name = v;
                self.has_changes = true;
            }
            DataFeedsMessage::SetUserId(v) => {
                self.edit_form.user_id = v;
                self.has_changes = true;
            }
            DataFeedsMessage::SetPassword(v) => {
                self.edit_form.password = v;
                self.has_changes = true;
            }
            DataFeedsMessage::SetAutoReconnect(v) => {
                self.edit_form.auto_reconnect = v;
                self.has_changes = true;
            }
            DataFeedsMessage::ToggleTickerDropdown => {
                self.ticker_dropdown_open = !self.ticker_dropdown_open;
            }
            DataFeedsMessage::CloseTickerDropdown => {
                self.ticker_dropdown_open = false;
            }
            DataFeedsMessage::ToggleTicker(ticker) => {
                if let Some(pos) = self
                    .edit_form
                    .subscribed_tickers
                    .iter()
                    .position(|t| t == &ticker)
                {
                    self.edit_form.subscribed_tickers.remove(pos);
                } else {
                    self.edit_form.subscribed_tickers.push(ticker);
                }
                self.has_changes = true;
            }
        }

        None
    }

    fn apply_form_to_feed(&self, feed: &mut DataFeed) {
        feed.name = self.edit_form.name.clone();
        feed.priority = self
            .edit_form
            .priority
            .parse::<u32>()
            .unwrap_or(feed.priority);

        match &mut feed.config {
            FeedConfig::Databento(cfg) => {
                cfg.cache_enabled = self.edit_form.cache_enabled;
                cfg.cache_max_days = self
                    .edit_form
                    .cache_max_days
                    .parse::<u32>()
                    .unwrap_or(cfg.cache_max_days);
            }
            FeedConfig::Rithmic(cfg) => {
                cfg.environment = self.edit_form.environment;
                cfg.system_name = self.edit_form.system_name.clone();
                cfg.user_id = self.edit_form.user_id.clone();
                cfg.auto_reconnect = self.edit_form.auto_reconnect;
                cfg.subscribed_tickers =
                    self.edit_form.subscribed_tickers.clone();
            }
        }
    }

    // ================================================================
    // Views
    // ================================================================

    pub fn view(&self) -> Element<'_, DataFeedsMessage> {
        let title_bar = container(text("Manage Connections").size(16))
            .padding([12, 16])
            .width(Length::Fill);

        let left_panel = self.view_left_panel();
        let right_panel = self.view_right_panel();

        let body = row![
            left_panel,
            rule::vertical(1).style(style::split_ruler),
            right_panel,
        ]
        .height(420);

        let content = column![
            title_bar,
            rule::horizontal(1).style(style::split_ruler),
            body,
        ];

        container(content)
            .width(650)
            .style(style::dashboard_modal)
            .into()
    }

    fn view_left_panel(&self) -> Element<'_, DataFeedsMessage> {
        let feeds = &self.feeds_snapshot;

        let historical = feeds.historical_feeds();
        let realtime = feeds.realtime_feeds();

        let mut feed_list = column![].spacing(2);

        // Datasets section
        if !historical.is_empty() {
            feed_list = feed_list.push(section_header("Datasets"));
            for feed in &historical {
                let is_selected = self.selected_feed == Some(feed.id);
                feed_list = feed_list.push(self.view_feed_item(feed, is_selected));
            }
        }

        // Connections section
        if !realtime.is_empty() {
            feed_list = feed_list.push(section_header("Connections"));
            for feed in feeds.feeds_by_priority() {
                if feed.is_realtime() {
                    let is_selected = self.selected_feed == Some(feed.id);
                    feed_list = feed_list.push(self.view_feed_item(feed, is_selected));
                }
            }
        }

        if feeds.total_count() == 0 && !self.is_creating {
            feed_list = feed_list.push(
                container(text("No connections").size(12))
                    .padding([16, 12])
                    .width(Length::Fill)
                    .align_x(Alignment::Center),
            );
        }

        // "+" and "-" buttons
        let add_button = button(text("+").size(14).align_x(Alignment::Center))
            .width(28)
            .height(28)
            .on_press(DataFeedsMessage::ToggleAddPopup);

        let remove_button: Option<Element<'_, DataFeedsMessage>> =
            self.selected_feed.map(|id| {
                button(
                    text("\u{2212}").size(14).align_x(Alignment::Center),
                )
                .width(28)
                .height(28)
                .on_press(DataFeedsMessage::RemoveFeed(id))
                .style(style::button::secondary)
                .into()
            });

        let mut button_row = row![add_button].spacing(4);
        if let Some(rm) = remove_button {
            button_row = button_row.push(rm);
        }

        let add_area: Element<'_, DataFeedsMessage> = if self.add_popup_open {
            let popup = container(
                column![
                    button(text("Historical").size(12))
                        .width(Length::Fill)
                        .on_press(DataFeedsMessage::OpenHistoricalDownload,)
                        .padding([4, 12]),
                    button(text("Realtime").size(12))
                        .width(Length::Fill)
                        .on_press(DataFeedsMessage::AddRealtime)
                        .padding([4, 12]),
                ]
                .spacing(2),
            )
            .padding(4)
            .style(style::dashboard_modal);

            stack![
                mouse_area(
                    container(space::horizontal())
                        .width(200)
                        .height(Length::Fill)
                )
                .on_press(DataFeedsMessage::CloseAddPopup),
                column![
                    space::vertical().height(Length::Fill),
                    popup,
                    container(button_row).padding([6, 8]),
                ],
            ]
            .height(Length::Fill)
            .into()
        } else {
            column![
                space::vertical().height(Length::Fill),
                rule::horizontal(1).style(style::split_ruler),
                container(button_row).padding([6, 8]),
            ]
            .into()
        };

        column![
            scrollable(feed_list.padding([4, 0])).height(Length::Fill),
            add_area,
        ]
        .width(200)
        .into()
    }

    fn view_feed_item<'a>(
        &self,
        feed: &'a DataFeed,
        is_selected: bool,
    ) -> Element<'a, DataFeedsMessage> {
        let indicator: Element<'a, DataFeedsMessage> = if feed.is_historical() {
            // Small "DB" label for datasets
            container(text("DB").size(8).align_x(Alignment::Center))
                .width(18)
                .height(18)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|_theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(Color::from_rgba(
                        0.3, 0.6, 1.0, 0.3,
                    ))),
                    border: iced::Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        } else {
            // Status dot for connections
            let status_color = match &feed.status {
                FeedStatus::Connected => Color::from_rgb(0.2, 0.8, 0.2),
                FeedStatus::Connecting => Color::from_rgb(0.9, 0.7, 0.1),
                FeedStatus::Downloading { .. } => Color::from_rgb(0.3, 0.6, 1.0),
                FeedStatus::Error(_) => Color::from_rgb(0.9, 0.2, 0.2),
                FeedStatus::Disconnected => Color::from_rgb(0.5, 0.5, 0.5),
            };

            container(space::horizontal().width(8).height(8))
                .style(move |_theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(status_color)),
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        };

        let info = column![
            text(&feed.name).size(13),
            text(feed.provider.display_name()).size(10),
        ]
        .spacing(1);

        let item_content = row![indicator, info].spacing(8).align_y(Alignment::Center);

        let feed_id = feed.id;
        let btn = button(item_content)
            .width(Length::Fill)
            .on_press(DataFeedsMessage::SelectFeed(feed_id))
            .padding([6, 12]);

        if is_selected {
            btn.style(style::button::primary).into()
        } else {
            btn.style(style::button::list_item).into()
        }
    }

    fn view_right_panel(&self) -> Element<'_, DataFeedsMessage> {
        let feeds = &self.feeds_snapshot;

        match self.selected_feed {
            None => container(text("Select a connection or click + to add one").size(13))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into(),

            Some(id) => {
                if let Some(feed) = feeds.get(id) {
                    match &feed.kind {
                        FeedKind::Historical(info) => self.view_historical_panel(feed, info),
                        FeedKind::Realtime => self.view_edit_form(feed),
                    }
                } else {
                    container(text("Feed not found").size(13))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .into()
                }
            }
        }
    }

    fn view_historical_panel<'a>(
        &'a self,
        feed: &'a DataFeed,
        info: &'a HistoricalDatasetInfo,
    ) -> Element<'a, DataFeedsMessage> {
        // Editable name
        let name_field = column![
            text("Name").size(12),
            text_input("Dataset name", &self.edit_form.name)
                .on_input(DataFeedsMessage::SetName)
                .size(13),
        ]
        .spacing(4);

        // Info row (read-only)
        let info_row = column![
            row![
                text("Provider:").size(11),
                text(feed.provider.display_name()).size(11),
                space::horizontal().width(12),
                text("Ticker:").size(11),
                text(&info.ticker).size(11),
            ]
            .spacing(4),
            row![
                text("Range:").size(11),
                text(format!(
                    "{} - {}",
                    info.date_range.start.format("%b %d, %Y"),
                    info.date_range.end.format("%b %d, %Y")
                ))
                .size(11),
            ]
            .spacing(4),
            row![
                text("Schema:").size(11),
                text(&info.schema).size(11),
                if let Some(count) = info.trade_count {
                    Element::from(
                        row![
                            space::horizontal().width(12),
                            text("Trades:").size(11),
                            text(format_count(count)).size(11),
                        ]
                        .spacing(4),
                    )
                } else {
                    space::horizontal().width(0).into()
                },
            ]
            .spacing(4),
        ]
        .spacing(2);

        // Price line chart
        let chart_section: Element<'_, DataFeedsMessage> =
            if let Some(ref preview) = self.preview_data {
                if !preview.price_line.is_empty() {
                    let chart = PriceLineChart {
                        points: preview.price_line.clone(),
                    };
                    container(canvas::Canvas::new(chart).width(Length::Fill).height(120))
                        .style(style::modal_container)
                        .into()
                } else {
                    container(text("No price data available").size(11))
                        .height(60)
                        .width(Length::Fill)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .style(style::modal_container)
                        .into()
                }
            } else if self.preview_loading {
                container(text("Loading preview...").size(11))
                    .height(60)
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(style::modal_container)
                    .into()
            } else {
                container(text("No preview available").size(11))
                    .height(60)
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(style::modal_container)
                    .into()
            };

        // Trade table
        let trade_table: Element<'_, DataFeedsMessage> = if let Some(ref preview) =
            self.preview_data
        {
            if !preview.trades.is_empty() {
                let header = row![
                    text("Time").size(10).width(Length::FillPortion(3)),
                    text("Price").size(10).width(Length::FillPortion(2)),
                    text("Size").size(10).width(Length::FillPortion(1)),
                    text("Side").size(10).width(Length::FillPortion(1)),
                ]
                .spacing(4)
                .padding([2, 4]);

                let mut rows = column![header].spacing(1);
                for (i, trade) in preview.trades.iter().take(50).enumerate() {
                    let side_style = if trade.side == "Buy" {
                        Color::from_rgb(0.2, 0.8, 0.2)
                    } else {
                        Color::from_rgb(0.9, 0.2, 0.2)
                    };

                    let trade_row = row![
                        text(&trade.time).size(10).width(Length::FillPortion(3)),
                        text(&trade.price).size(10).width(Length::FillPortion(2)),
                        text(&trade.size).size(10).width(Length::FillPortion(1)),
                        text(&trade.side)
                            .size(10)
                            .width(Length::FillPortion(1))
                            .style(move |_: &iced::Theme| {
                                iced::widget::text::Style {
                                    color: Some(side_style),
                                }
                            },),
                    ]
                    .spacing(4)
                    .padding([1, 4]);

                    rows = rows.push(trade_row);
                }

                if preview.total_trades > 50 {
                    rows = rows.push(
                        text(format!("... and {} more trades", preview.total_trades - 50)).size(10),
                    );
                }

                scrollable(rows).height(120).into()
            } else {
                space::vertical().height(0).into()
            }
        } else {
            space::vertical().height(0).into()
        };

        // Delete button
        let feed_id = feed.id;
        let delete_section = column![
            rule::horizontal(1).style(style::split_ruler),
            container(
                button(text("Delete Dataset").size(12))
                    .on_press(DataFeedsMessage::RemoveFeed(feed_id))
                    .padding([4, 12]),
            )
            .align_x(Alignment::End)
            .width(Length::Fill),
        ]
        .spacing(8);

        let form_content = column![
            name_field,
            info_row,
            chart_section,
            trade_table,
            delete_section,
        ]
        .spacing(10)
        .padding([12, 16]);

        scrollable(form_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_edit_form<'a>(&'a self, feed: &'a DataFeed) -> Element<'a, DataFeedsMessage> {
        // Name + Type on the same row (3/4 name, 1/4 type)
        let type_col: Element<'_, DataFeedsMessage> = if self.is_creating {
            column![
                text("Type").size(12),
                pick_list(
                    FeedProvider::ALL,
                    self.edit_form.provider,
                    DataFeedsMessage::SetProvider,
                )
                .text_size(13),
            ]
            .spacing(4)
            .width(Length::FillPortion(1))
            .into()
        } else {
            column![
                text("Type").size(12),
                text_input("", feed.provider.display_name())
                    .size(13),
            ]
            .spacing(4)
            .width(Length::FillPortion(1))
            .into()
        };

        let name_type_row = row![
            column![
                text("Name").size(12),
                text_input("Connection name", &self.edit_form.name)
                    .on_input(DataFeedsMessage::SetName)
                    .size(13),
            ]
            .spacing(4)
            .width(Length::FillPortion(3)),
            type_col,
        ]
        .spacing(8);

        // Provider-specific fields
        let provider_fields: Element<'_, DataFeedsMessage> = match self.edit_form.provider {
            Some(FeedProvider::Databento) => self.view_databento_fields(),
            Some(FeedProvider::Rithmic) => self.view_rithmic_fields(),
            None => space::vertical().height(0).into(),
        };

        let form_content = column![
            name_type_row,
            rule::horizontal(1).style(style::split_ruler),
            provider_fields,
        ]
        .spacing(12)
        .padding([12, 16]);

        // Footer
        let footer = container(
            row![
                space::horizontal().width(Length::Fill),
                button(text("Cancel").size(13))
                    .on_press(DataFeedsMessage::CancelEdit)
                    .padding([4, 12])
                    .style(style::button::secondary),
                button(text("Save").size(13))
                    .on_press(DataFeedsMessage::SaveFeed)
                    .padding([4, 12])
                    .style(style::button::primary),
            ]
            .spacing(8),
        )
        .padding([8, 16]);

        column![
            scrollable(form_content).height(Length::Fill),
            rule::horizontal(1).style(style::split_ruler),
            footer,
        ]
        .width(Length::Fill)
        .into()
    }

    fn view_databento_fields(&self) -> Element<'_, DataFeedsMessage> {
        let has_saved_key = data::SecretsManager::new().has_api_key(data::ApiProvider::Databento);
        let key_placeholder = if has_saved_key {
            "API key saved (leave blank to keep)"
        } else {
            "Enter Databento API key"
        };

        let api_key_field = column![
            text("API Key").size(12),
            text_input(key_placeholder, &self.edit_form.api_key)
                .on_input(DataFeedsMessage::SetApiKey)
                .secure(true)
                .size(13),
        ]
        .spacing(4);

        let cache_toggle = row![
            text("Enable caching").size(12),
            space::horizontal().width(Length::Fill),
            button(
                text(if self.edit_form.cache_enabled {
                    "On"
                } else {
                    "Off"
                })
                .size(11)
            )
            .on_press(DataFeedsMessage::SetCacheEnabled(
                !self.edit_form.cache_enabled,
            ))
            .padding([2, 8]),
        ]
        .align_y(Alignment::Center);

        let cache_days = column![
            text("Cache max days").size(12),
            text_input("90", &self.edit_form.cache_max_days)
                .on_input(DataFeedsMessage::SetCacheMaxDays)
                .size(13),
        ]
        .spacing(4);

        column![
            text("Databento Settings").size(14),
            api_key_field,
            cache_toggle,
            cache_days,
        ]
        .spacing(8)
        .into()
    }

    fn view_rithmic_fields(&self) -> Element<'_, DataFeedsMessage> {
        // Environment + System Name on the same row
        let env_options: Vec<String> = RithmicEnvironment::ALL
            .iter()
            .map(|e| e.to_string())
            .collect();
        let selected_env = Some(self.edit_form.environment.to_string());

        let system_names = RITHMIC_SYSTEM_NAMES;
        let system_name_options: Vec<String> =
            system_names.iter().map(|s| s.to_string()).collect();
        let selected_system = if self.edit_form.system_name.is_empty() {
            None
        } else {
            Some(self.edit_form.system_name.clone())
        };

        let env_system_row = row![
            column![
                text("Environment").size(12),
                pick_list(env_options, selected_env, |selected| {
                    let env = RithmicEnvironment::ALL
                        .iter()
                        .find(|e| e.to_string() == selected)
                        .copied()
                        .unwrap_or(RithmicEnvironment::Demo);
                    DataFeedsMessage::SetEnvironment(env)
                })
                .text_size(12),
            ]
            .spacing(4)
            .width(Length::FillPortion(1)),
            column![
                text("System Name").size(12),
                pick_list(
                    system_name_options,
                    selected_system,
                    DataFeedsMessage::SetSystemName,
                )
                .text_size(12),
            ]
            .spacing(4)
            .width(Length::FillPortion(2)),
        ]
        .spacing(8);

        let user_id = column![
            text("User ID").size(12),
            text_input("Your Rithmic user ID", &self.edit_form.user_id,)
                .on_input(DataFeedsMessage::SetUserId)
                .size(13),
        ]
        .spacing(4);

        let password_field = {
            let has_saved = data::SecretsManager::new()
                .has_api_key(data::ApiProvider::Rithmic);
            let placeholder = if has_saved {
                "Password saved (leave blank to keep)"
            } else {
                "Enter password"
            };

            column![
                text("Password").size(12),
                text_input(placeholder, &self.edit_form.password)
                    .on_input(DataFeedsMessage::SetPassword)
                    .secure(true)
                    .size(13),
            ]
            .spacing(4)
        };

        // Subscribed tickers as multi-select dropdown (pick_list style)
        let selected_tickers = &self.edit_form.subscribed_tickers;
        let trigger_label = if selected_tickers.is_empty() {
            "Select tickers...".to_string()
        } else {
            selected_tickers.join(", ")
        };
        let trigger_text_style = if selected_tickers.is_empty() {
            // Placeholder style — dimmed
            |theme: &iced::Theme| iced::widget::text::Style {
                color: Some(
                    theme.extended_palette().background.strong.color,
                ),
            }
        } else {
            |theme: &iced::Theme| iced::widget::text::Style {
                color: Some(
                    theme.extended_palette().background.base.text,
                ),
            }
        };

        let trigger_btn = button(
            row![
                text(trigger_label)
                    .size(12)
                    .style(trigger_text_style),
                space::horizontal().width(Length::Fill),
                text("\u{25BC}").size(8), // down triangle ▼
            ]
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([6, 10])
        .style(style::button::pick_list_trigger)
        .on_press(DataFeedsMessage::ToggleTickerDropdown);

        let tickers_field = if self.ticker_dropdown_open {
            let mut items = column![].spacing(0);
            for &(symbol, _label) in RITHMIC_TICKERS {
                let is_checked = selected_tickers
                    .iter()
                    .any(|t| t == symbol);
                let item_text = if is_checked {
                    format!("\u{2713} {}", symbol) // ✓ ES
                } else {
                    format!("   {}", symbol)
                };
                let item = button(text(item_text).size(12))
                    .width(Length::Fill)
                    .padding([5, 10])
                    .style(style::button::pick_list_item)
                    .on_press(DataFeedsMessage::ToggleTicker(
                        symbol.to_string(),
                    ));
                items = items.push(item);
            }

            let dropdown = container(
                scrollable(items).height(Length::Shrink),
            )
            .max_height(200)
            .style(style::dropdown_container);

            column![
                text("Subscribed Tickers").size(12),
                trigger_btn,
                dropdown,
            ]
            .spacing(4)
        } else {
            column![
                text("Subscribed Tickers").size(12),
                trigger_btn,
            ]
            .spacing(4)
        };

        let reconnect_toggle = row![
            text("Auto-reconnect").size(12),
            space::horizontal().width(Length::Fill),
            button(
                text(if self.edit_form.auto_reconnect {
                    "On"
                } else {
                    "Off"
                })
                .size(11)
            )
            .on_press(DataFeedsMessage::SetAutoReconnect(
                !self.edit_form.auto_reconnect,
            ))
            .padding([2, 8]),
        ]
        .align_y(Alignment::Center);

        column![
            text("Rithmic Settings").size(14),
            env_system_row,
            user_id,
            password_field,
            tickers_field,
            reconnect_toggle,
        ]
        .spacing(8)
        .into()
    }
}

/// Predefined Rithmic system names
const RITHMIC_SYSTEM_NAMES: &[&str] = &[
    "Rithmic Paper Trading",
    "Rithmic 01",
    "Rithmic Test",
];

/// Tickers available for Rithmic subscription
const RITHMIC_TICKERS: &[(&str, &str)] = &[
    ("ES", "E-mini S&P 500"),
    ("NQ", "E-mini Nasdaq-100"),
    ("YM", "E-mini Dow"),
    ("RTY", "E-mini Russell 2000"),
    ("CL", "Crude Oil"),
    ("GC", "Gold"),
    ("SI", "Silver"),
    ("ZN", "10-Year T-Note"),
    ("ZB", "30-Year T-Bond"),
    ("ZF", "5-Year T-Note"),
    ("NG", "Natural Gas"),
    ("HG", "Copper"),
];

fn section_header(label: &str) -> Element<'_, DataFeedsMessage> {
    container(text(label).size(11))
        .padding(padding::top(6).right(12).bottom(2).left(12))
        .width(Length::Fill)
        .into()
}

fn format_count(count: usize) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}
