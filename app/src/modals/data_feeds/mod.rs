//! Data Feeds Modal
//!
//! Split-pane dialog for managing data feed connections. Left panel shows
//! the feed list split into "Datasets" and "Connections" sections, right
//! panel shows the edit form for realtime connections or a preview panel
//! for historical datasets.

mod preview;
mod view;

pub use preview::PreviewData;

use data::{
    self, Connection, ConnectionConfig, ConnectionManager, ConnectionProvider, ConnectionStatus,
    FeedId, HistoricalDatasetInfo, RithmicEnvironment, RithmicServer,
};

// ── DataFeedsModal ───────────────────────────────────────────────────────

/// Data Feeds modal state
#[derive(Debug, Clone)]
pub struct DataFeedsModal {
    selected_feed: Option<FeedId>,
    pub(super) edit_form: EditForm,
    pub(super) is_creating: bool,
    has_changes: bool,
    pub(super) feeds_snapshot: ConnectionManager,
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
    pub(super) provider: Option<ConnectionProvider>,
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
    pub(super) user_id: String,
    pub(super) password: String,
    pub(super) auto_reconnect: bool,
    pub(super) subscribed_tickers: Vec<String>,
    pub(super) backfill_days: i64,
    pub(super) tickers_dropdown_open: bool,
    pub(super) system_names_loading: bool,
    pub(super) available_system_names: Vec<String>,
    pub(super) available_tickers: Vec<String>,
    // General
    pub(super) auto_connect: bool,
    // Cached credential status (avoid SecretsManager I/O in view)
    pub(super) has_saved_api_key: bool,
    pub(super) has_saved_password: bool,
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
            user_id: String::new(),
            password: String::new(),
            auto_reconnect: true,
            subscribed_tickers: Vec::new(),
            backfill_days: 1,
            tickers_dropdown_open: false,
            system_names_loading: false,
            available_system_names: Vec::new(),
            available_tickers: Vec::new(),
            auto_connect: false,
            has_saved_api_key: false,
            has_saved_password: false,
        }
    }
}

impl EditForm {
    fn from_feed(feed: &Connection) -> Self {
        let mut form = Self {
            provider: Some(feed.provider),
            name: feed.name.clone(),
            priority: feed.priority.to_string(),
            auto_connect: feed.auto_connect,
            ..Default::default()
        };

        match &feed.config {
            ConnectionConfig::Databento(cfg) => {
                form.cache_enabled = cfg.cache_enabled;
                form.cache_max_days = cfg.cache_max_days.to_string();
            }
            ConnectionConfig::Rithmic(cfg) => {
                form.environment = cfg.environment;
                form.server = cfg.server;
                form.system_name = cfg.system_name.clone();
                form.user_id = cfg.user_id.clone();
                form.auto_reconnect = cfg.auto_reconnect;
                form.subscribed_tickers = cfg.subscribed_tickers.clone();
                form.backfill_days = cfg.backfill_days;
            }
        }

        form
    }

    fn for_provider(provider: ConnectionProvider) -> Self {
        match provider {
            ConnectionProvider::Databento => Self {
                provider: Some(ConnectionProvider::Databento),
                name: "New Connection".to_string(),
                priority: "10".to_string(),
                ..Default::default()
            },
            ConnectionProvider::Rithmic => Self {
                provider: Some(ConnectionProvider::Rithmic),
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
    AddRithmic,
    AddDatabento,
    // Right panel - form
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
    SetUserId(String),
    SetPassword(String),
    SetAutoReconnect(bool),
    SetAutoConnect(bool),
    SetBackfillDays(i64),
    ToggleTicker(String),
    ToggleTickersExpanded,
    SystemNamesLoaded(RithmicServer, Result<Vec<String>, String>),
    AvailableTickersLoaded(Result<Vec<String>, String>),
    // Connection actions
    ConnectFeed(FeedId),
    DisconnectFeed(FeedId),
    // Close
    Close,
    // Status updates
    FeedStatusChanged(FeedId, ConnectionStatus),
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
        provider: crate::config::secrets::ApiProvider,
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
            feeds_snapshot: ConnectionManager::default(),
            add_popup_open: false,
            preview_data: None,
            preview_loading: false,
        }
    }

    pub fn sync_snapshot(&mut self, manager: &ConnectionManager) {
        self.feeds_snapshot = manager.clone();
    }

    /// Returns the currently selected feed ID, if any.
    pub fn selected_feed_id(&self) -> Option<FeedId> {
        self.selected_feed
    }

    /// Cache credential status so views don't need SecretsManager I/O.
    pub fn set_credential_status(&mut self, has_api_key: bool, has_password: bool) {
        self.edit_form.has_saved_api_key = has_api_key;
        self.edit_form.has_saved_password = has_password;
    }

    pub fn update(
        &mut self,
        message: DataFeedsMessage,
        feed_manager: &mut ConnectionManager,
    ) -> Vec<Action> {
        match message {
            DataFeedsMessage::Close => {
                return vec![Action::Close];
            }

            // ── Left panel ────────────────────────────────────────────────
            DataFeedsMessage::SelectFeed(id) => {
                if let Some(feed) = feed_manager.get(id) {
                    self.edit_form = EditForm::from_feed(feed);
                    self.edit_form.tickers_dropdown_open = false;
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
                            return vec![Action::LoadPreview(id, info.clone())];
                        }
                    } else {
                        self.preview_data = None;
                        self.preview_loading = false;
                    }

                    // Probe system names for Rithmic feeds
                    if feed.provider == ConnectionProvider::Rithmic {
                        self.edit_form.system_names_loading = true;
                        return vec![Action::ProbeSystemNames(self.edit_form.server)];
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
            DataFeedsMessage::AddRithmic => {
                self.add_popup_open = false;
                let feed = Connection::new_rithmic("New Connection");
                let id = feed.id;
                feed_manager.add(feed);
                self.selected_feed = Some(id);
                self.is_creating = true;
                self.has_changes = false;
                self.edit_form = EditForm::for_provider(ConnectionProvider::Rithmic);
                self.edit_form.system_names_loading = true;
                return vec![
                    Action::FeedsUpdated,
                    Action::ProbeSystemNames(self.edit_form.server),
                ];
            }
            DataFeedsMessage::AddDatabento => {
                self.add_popup_open = false;
                return vec![Action::OpenHistoricalDownload];
            }

            // ── Right panel: form ──────────────────────────────────────────
            DataFeedsMessage::SaveFeed => {
                if let Some(id) = self.selected_feed {
                    let provider = self.edit_form.provider;

                    // Apply form data to the feed config before persisting
                    if let Some(feed) = feed_manager.get_mut(id) {
                        self.apply_form_to_feed(feed);
                    }

                    self.is_creating = false;
                    self.has_changes = false;
                    self.edit_form.tickers_dropdown_open = false;

                    // Delegate credential persistence to the parent (modal has no keyring access)
                    let mut actions = vec![Action::FeedsUpdated, Action::Close];

                    if provider == Some(ConnectionProvider::Databento)
                        && !self.edit_form.api_key.is_empty()
                    {
                        actions.insert(
                            0,
                            Action::SaveApiKey {
                                provider: crate::config::secrets::ApiProvider::Databento,
                                key: self.edit_form.api_key.clone(),
                            },
                        );
                    }
                    if provider == Some(ConnectionProvider::Rithmic)
                        && !self.edit_form.password.is_empty()
                    {
                        actions.insert(
                            0,
                            Action::SaveFeedPassword {
                                feed_id: id,
                                password: self.edit_form.password.clone(),
                            },
                        );
                    }

                    return actions;
                }
            }
            DataFeedsMessage::CancelEdit => {
                self.edit_form.tickers_dropdown_open = false;
                if self.is_creating {
                    if let Some(id) = self.selected_feed {
                        feed_manager.remove(id);
                    }
                    self.selected_feed = None;
                    self.is_creating = false;
                    self.has_changes = false;
                    return vec![Action::FeedsUpdated, Action::Close];
                } else if let Some(id) = self.selected_feed
                    && let Some(feed) = feed_manager.get(id)
                {
                    self.edit_form = EditForm::from_feed(feed);
                    self.has_changes = false;
                }
                return vec![Action::Close];
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
            DataFeedsMessage::SetEnvironment(env) => {
                self.edit_form.environment = env;
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
            DataFeedsMessage::SetUserId(v) => {
                self.edit_form.user_id = v;
                self.has_changes = true;
            }
            DataFeedsMessage::SetPassword(v) => {
                self.edit_form.password = v;
                self.has_changes = true;
            }
            DataFeedsMessage::SetBackfillDays(days) => {
                self.edit_form.backfill_days = days;
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
            DataFeedsMessage::ToggleTickersExpanded => {
                self.edit_form.tickers_dropdown_open = !self.edit_form.tickers_dropdown_open;
            }
            DataFeedsMessage::SystemNamesLoaded(server, result) => {
                // Only apply if server still matches current form
                if self.edit_form.server == server {
                    self.edit_form.system_names_loading = false;
                    match result {
                        Ok(names) => {
                            // Auto-select first system name if none set
                            if self.edit_form.system_name.is_empty()
                                && let Some(first) = names.first()
                            {
                                self.edit_form.system_name = first.clone();
                            }
                            self.edit_form.available_system_names = names;
                        }
                        Err(e) => {
                            log::warn!("Failed to probe system names: {}", e);
                            self.edit_form.available_system_names.clear();
                        }
                    }
                }
            }
            DataFeedsMessage::AvailableTickersLoaded(result) => match result {
                Ok(tickers) => {
                    self.edit_form.available_tickers = tickers;
                }
                Err(e) => {
                    log::warn!("Failed to load available tickers: {}", e);
                }
            },
        }

        vec![]
    }

    fn apply_form_to_feed(&self, feed: &mut Connection) {
        feed.name = self.edit_form.name.clone();
        feed.priority = self
            .edit_form
            .priority
            .parse::<u32>()
            .unwrap_or(feed.priority);
        feed.auto_connect = self.edit_form.auto_connect;

        match &mut feed.config {
            ConnectionConfig::Databento(cfg) => {
                cfg.cache_enabled = self.edit_form.cache_enabled;
                cfg.cache_max_days = self
                    .edit_form
                    .cache_max_days
                    .parse::<u32>()
                    .unwrap_or(cfg.cache_max_days);
            }
            ConnectionConfig::Rithmic(cfg) => {
                cfg.environment = self.edit_form.environment;
                cfg.server = self.edit_form.server;
                cfg.system_name = self.edit_form.system_name.clone();
                cfg.user_id = self.edit_form.user_id.clone();
                cfg.auto_reconnect = self.edit_form.auto_reconnect;
                cfg.subscribed_tickers = self.edit_form.subscribed_tickers.clone();
                cfg.backfill_days = self.edit_form.backfill_days;
            }
        }
    }
}
