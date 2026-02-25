//! Data Feeds Modal
//!
//! Split-pane dialog for managing data feed connections. Left panel shows
//! the feed list split into "Datasets" and "Connections" sections, right
//! panel shows the edit form for realtime connections or a preview panel
//! for historical datasets.

mod preview;
mod view;

pub use preview::{PreviewData, TradePreviewRow};

use data::{
    self,
    feed::{
        DataFeed, DataFeedManager, DatabentoFeedConfig, FeedConfig, FeedId, FeedProvider,
        FeedStatus, HistoricalDatasetInfo, RithmicEnvironment, RithmicFeedConfig, RithmicServer,
    },
};

// ── DataFeedsModal ────────────────────────────────────────────────────

/// Data Feeds modal state
#[derive(Debug, Clone)]
pub struct DataFeedsModal {
    selected_feed: Option<FeedId>,
    pub(super) edit_form: EditForm,
    pub(super) is_creating: bool,
    has_changes: bool,
    pub(super) feeds_snapshot: DataFeedManager,
    /// "+" popup open
    pub(super) add_popup_open: bool,
    /// Preview data for selected historical feed
    pub(super) preview_data: Option<PreviewData>,
    pub(super) preview_loading: bool,
}

impl PartialEq for DataFeedsModal {
    fn eq(&self, other: &Self) -> bool {
        self.selected_feed == other.selected_feed
            && self.edit_form == other.edit_form
            && self.is_creating == other.is_creating
            && self.has_changes == other.has_changes
            && self.feeds_snapshot == other.feeds_snapshot
            && self.add_popup_open == other.add_popup_open
    }
}

/// Form state for editing a feed
#[derive(Debug, Clone, PartialEq)]
pub(super) struct EditForm {
    pub(super) provider: Option<FeedProvider>,
    pub(super) name: String,
    pub(super) priority: String,
    // Databento
    pub(super) api_key: String,
    pub(super) cache_enabled: bool,
    pub(super) cache_max_days: String,
    // Rithmic
    pub(super) environment: RithmicEnvironment,
    pub(super) server: RithmicServer,
    pub(super) system_name: String,
    pub(super) account_id: String,
    pub(super) user_id: String,
    pub(super) password: String,
    pub(super) auto_reconnect: bool,
    pub(super) subscribed_tickers: Vec<String>,
    pub(super) system_names_loading: bool,
    pub(super) available_system_names: Vec<String>,
    pub(super) available_tickers: Vec<String>,
    // General
    pub(super) auto_connect: bool,
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
            server: RithmicServer::default(),
            system_name: String::new(),
            account_id: String::new(),
            user_id: String::new(),
            password: String::new(),
            auto_reconnect: true,
            subscribed_tickers: Vec::new(),
            system_names_loading: false,
            available_system_names: Vec::new(),
            available_tickers: Vec::new(),
            auto_connect: false,
        }
    }
}

impl EditForm {
    fn from_feed(feed: &DataFeed) -> Self {
        let mut form = Self {
            provider: Some(feed.provider),
            name: feed.name.clone(),
            priority: feed.priority.to_string(),
            auto_connect: feed.auto_connect,
            ..Default::default()
        };

        match &feed.config {
            FeedConfig::Databento(cfg) => {
                form.cache_enabled = cfg.cache_enabled;
                form.cache_max_days = cfg.cache_max_days.to_string();
            }
            FeedConfig::Rithmic(cfg) => {
                form.environment = cfg.environment;
                // Migrate: try server enum first, fall back to URL lookup
                form.server = if cfg.server != RithmicServer::default()
                    || cfg.server_url.is_empty()
                {
                    cfg.server
                } else {
                    RithmicServer::from_url(&cfg.server_url)
                        .unwrap_or(cfg.server)
                };
                form.system_name = cfg.system_name.clone();
                form.account_id = cfg.account_id.clone();
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
                server: RithmicServer::Chicago,
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
    DeselectFeed,
    RemoveFeed(FeedId),
    // "+" popup
    ToggleAddPopup,
    CloseAddPopup,
    AddRealtime,
    OpenHistoricalDownload,
    // Right panel - form
    SetProvider(FeedProvider),
    SetName(String),
    SaveFeed,
    CancelEdit,
    // Databento fields
    SetApiKey(String),
    SetCacheEnabled(bool),
    SetCacheMaxDays(String),
    // Rithmic fields
    SetEnvironment(RithmicEnvironment),
    SetServer(RithmicServer),
    SetSystemName(String),
    SetAccountId(String),
    SetUserId(String),
    SetPassword(String),
    SetAutoReconnect(bool),
    SetAutoConnect(bool),
    ToggleTicker(String),
    SystemNamesLoaded(RithmicServer, Result<Vec<String>, String>),
    AvailableTickersLoaded(Result<Vec<String>, String>),
    // Connection actions
    ConnectFeed(FeedId),
    DisconnectFeed(FeedId),
    // Close
    Close,
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
    /// Persist an API credential via the OS keyring (parent has SecretsManager access).
    SaveApiKey {
        provider: data::ApiProvider,
        key: String,
    },
    /// Persist a per-connection Rithmic password keyed by feed ID.
    SaveFeedPassword {
        feed_id: FeedId,
        password: String,
    },
    /// Probe a Rithmic server for available system names (pre-login).
    ProbeSystemNames(RithmicServer),
    Close,
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
    ) -> Vec<Action> {
        match message {
            DataFeedsMessage::Close => {
                return vec![Action::Close];
            }

            // ── Left panel ────────────────────────────────────────────────
            DataFeedsMessage::SelectFeed(id) => {
                if let Some(feed) = feed_manager.get(id) {
                    self.edit_form = EditForm::from_feed(feed);
                    self.selected_feed = Some(id);
                    self.is_creating = false;
                    self.has_changes = false;

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
                            return vec![Action::LoadPreview(
                                id,
                                info.clone(),
                            )];
                        }
                    } else {
                        self.preview_data = None;
                        self.preview_loading = false;
                    }

                    // Probe system names for Rithmic feeds
                    if feed.provider == FeedProvider::Rithmic {
                        self.edit_form.system_names_loading = true;
                        return vec![Action::ProbeSystemNames(
                            self.edit_form.server,
                        )];
                    }
                }
            }
            DataFeedsMessage::DeselectFeed => {
                self.selected_feed = None;
                self.is_creating = false;
                self.has_changes = false;
                self.preview_data = None;
                self.preview_loading = false;
            }
            DataFeedsMessage::RemoveFeed(id) => {
                feed_manager.remove(id);
                if self.selected_feed == Some(id) {
                    self.selected_feed = None;
                    self.is_creating = false;
                    self.has_changes = false;
                    self.preview_data = None;
                }
                return vec![Action::FeedsUpdated];
            }

            // ── "+" popup ──────────────────────────────────────────────────
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
                self.edit_form.system_names_loading = true;
                return vec![
                    Action::FeedsUpdated,
                    Action::ProbeSystemNames(self.edit_form.server),
                ];
            }
            DataFeedsMessage::OpenHistoricalDownload => {
                self.add_popup_open = false;
                return vec![Action::OpenHistoricalDownload];
            }

            // ── Right panel: form ──────────────────────────────────────────
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
                            self.edit_form.server = RithmicServer::Chicago;
                            self.edit_form.system_name = String::new();
                            self.edit_form.account_id = String::new();
                            self.edit_form.user_id = String::new();
                            self.edit_form.password = String::new();
                            self.edit_form.auto_reconnect = true;
                            self.edit_form.subscribed_tickers = Vec::new();
                            self.edit_form.available_system_names = Vec::new();
                            self.edit_form.available_tickers = Vec::new();
                            self.edit_form.system_names_loading = true;
                            self.edit_form.priority = "5".to_string();
                            return vec![Action::ProbeSystemNames(
                                RithmicServer::Chicago,
                            )];
                        }
                    }
                    if let Some(id) = self.selected_feed
                        && let Some(feed) = feed_manager.get_mut(id)
                    {
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
                    self.has_changes = true;
                }
            }
            DataFeedsMessage::SaveFeed => {
                if let Some(id) = self.selected_feed {
                    let provider = self.edit_form.provider;

                    // Apply form data to the feed config before persisting
                    if let Some(feed) = feed_manager.get_mut(id) {
                        self.apply_form_to_feed(feed);
                    }

                    self.is_creating = false;
                    self.has_changes = false;

                    // Delegate credential persistence to the parent (modal has no keyring access)
                    if provider == Some(FeedProvider::Databento)
                        && !self.edit_form.api_key.is_empty()
                    {
                        return vec![Action::SaveApiKey {
                            provider: data::ApiProvider::Databento,
                            key: self.edit_form.api_key.clone(),
                        }];
                    }
                    if provider == Some(FeedProvider::Rithmic)
                        && !self.edit_form.password.is_empty()
                    {
                        return vec![Action::SaveFeedPassword {
                            feed_id: id,
                            password: self.edit_form.password.clone(),
                        }];
                    }

                    return vec![Action::FeedsUpdated];
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
                    return vec![Action::FeedsUpdated];
                } else if let Some(id) = self.selected_feed
                    && let Some(feed) = feed_manager.get(id)
                {
                    self.edit_form = EditForm::from_feed(feed);
                    self.has_changes = false;
                }
            }

            // ── Connection actions ─────────────────────────────────────────
            DataFeedsMessage::ConnectFeed(id) => {
                return vec![Action::ConnectFeed(id)];
            }
            DataFeedsMessage::DisconnectFeed(id) => {
                return vec![Action::DisconnectFeed(id)];
            }

            // ── Status updates ────────────────────────────────────────────
            DataFeedsMessage::FeedStatusChanged(id, status) => {
                feed_manager.set_status(id, status);
            }

            // ── Preview ────────────────────────────────────────────────────
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

            // ── Form field setters ────────────────────────────────────────
            DataFeedsMessage::SetName(v) => {
                self.edit_form.name = v;
                self.has_changes = true;

                // For historical feeds, also update the name in the
                // manager immediately
                if let Some(id) = self.selected_feed
                    && let Some(feed) = feed_manager.get_mut(id)
                    && feed.is_historical()
                {
                    feed.name = self.edit_form.name.clone();
                    return vec![Action::FeedsUpdated];
                }
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
            DataFeedsMessage::SetServer(server) => {
                self.edit_form.server = server;
                self.edit_form.system_names_loading = true;
                self.edit_form.available_system_names.clear();
                self.edit_form.system_name.clear();
                self.has_changes = true;
                return vec![Action::ProbeSystemNames(server)];
            }
            DataFeedsMessage::SetSystemName(v) => {
                self.edit_form.system_name = v;
                self.has_changes = true;
            }
            DataFeedsMessage::SetAccountId(v) => {
                self.edit_form.account_id = v;
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
            DataFeedsMessage::SetAutoConnect(v) => {
                self.edit_form.auto_connect = v;
                self.has_changes = true;

                // Apply immediately for historical feeds (no Save button)
                if let Some(id) = self.selected_feed
                    && let Some(feed) = feed_manager.get_mut(id)
                    && feed.is_historical()
                {
                    feed.auto_connect = v;
                    return vec![Action::FeedsUpdated];
                }
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
            DataFeedsMessage::SystemNamesLoaded(server, result) => {
                // Only apply if server still matches current form
                if self.edit_form.server == server {
                    self.edit_form.system_names_loading = false;
                    match result {
                        Ok(names) => {
                            // Auto-select first system name if none set
                            if self.edit_form.system_name.is_empty() {
                                if let Some(first) = names.first() {
                                    self.edit_form.system_name =
                                        first.clone();
                                }
                            }
                            self.edit_form.available_system_names = names;
                        }
                        Err(e) => {
                            log::warn!(
                                "Failed to probe system names: {}",
                                e
                            );
                            self.edit_form.available_system_names.clear();
                        }
                    }
                }
            }
            DataFeedsMessage::AvailableTickersLoaded(result) => {
                match result {
                    Ok(tickers) => {
                        self.edit_form.available_tickers = tickers;
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to load available tickers: {}",
                            e
                        );
                    }
                }
            }
        }

        vec![]
    }

    fn apply_form_to_feed(&self, feed: &mut DataFeed) {
        feed.name = self.edit_form.name.clone();
        feed.priority = self
            .edit_form
            .priority
            .parse::<u32>()
            .unwrap_or(feed.priority);
        feed.auto_connect = self.edit_form.auto_connect;

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
                cfg.server = self.edit_form.server;
                cfg.system_name = self.edit_form.system_name.clone();
                cfg.account_id = self.edit_form.account_id.clone();
                cfg.user_id = self.edit_form.user_id.clone();
                cfg.auto_reconnect = self.edit_form.auto_reconnect;
                cfg.subscribed_tickers = self.edit_form.subscribed_tickers.clone();
            }
        }
    }
}
