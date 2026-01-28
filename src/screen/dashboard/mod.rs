pub mod pane;
pub mod panel;
pub mod sidebar;
pub mod tickers_table;

pub use sidebar::Sidebar;

use super::DashboardError;
use crate::{
    chart,
    modal::pane::{Modal, data_management::{CacheStatus, DownloadProgress}},
    screen::dashboard::tickers_table::TickersTable,
    style,
    widget::toast::Toast,
    window::{self, Window},
};
use data::{
    ChartConfig, ChartData, ChartState, ContentKind, DateRange, LinkGroup, LoadingStatus, UserTimezone,
    WindowSpec,
};
use exchange::FuturesTickerInfo;

use iced::{
    Element, Length, Task, Vector,
    widget::{
        PaneGrid, center, container,
        pane_grid::{self, Configuration},
    },
};
use std::{collections::HashMap, time::Instant, vec};

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
}

pub struct Dashboard {
    pub panes: pane_grid::State<pane::State>,
    pub focus: Option<(window::Id, pane_grid::Pane)>,
    pub popout: HashMap<window::Id, (pane_grid::State<pane::State>, WindowSpec)>,
    /// Chart states by pane ID
    pub charts: HashMap<uuid::Uuid, ChartState>,
    /// Market data service for async loading (None when API key not configured)
    pub market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
    /// Crosshair positions by link group
    pub crosshair_positions: HashMap<data::LinkGroup, (u64, f32)>, // (timestamp, price)
    /// Downloaded tickers registry (tracks which tickers have data and their ranges)
    pub downloaded_tickers: std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
    layout_id: uuid::Uuid,
}

impl Dashboard {
    /// Create a new Dashboard with the given market data service and registry
    pub fn new(
        market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
        downloaded_tickers: std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
    ) -> Self {
        Self {
            panes: pane_grid::State::with_configuration(Self::default_pane_config()),
            focus: None,
            charts: HashMap::new(),
            market_data_service,
            popout: HashMap::new(),
            crosshair_positions: HashMap::new(),
            downloaded_tickers,
            layout_id: uuid::Uuid::new_v4(),
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
        schema: exchange::DatabentoSchema,
        date_range: data::DateRange,
    },
    DownloadData {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: data::DateRange,
    },
}

impl Dashboard {
    fn default_pane_config() -> Configuration<pane::State> {
        Configuration::Split {
            axis: pane_grid::Axis::Vertical,
            ratio: 0.8,
            a: Box::new(Configuration::Split {
                axis: pane_grid::Axis::Horizontal,
                ratio: 0.4,
                a: Box::new(Configuration::Split {
                    axis: pane_grid::Axis::Vertical,
                    ratio: 0.5,
                    a: Box::new(Configuration::Pane(pane::State::default())),
                    b: Box::new(Configuration::Pane(pane::State::default())),
                }),
                b: Box::new(Configuration::Split {
                    axis: pane_grid::Axis::Vertical,
                    ratio: 0.5,
                    a: Box::new(Configuration::Pane(pane::State::default())),
                    b: Box::new(Configuration::Pane(pane::State::default())),
                }),
            }),
            b: Box::new(Configuration::Pane(pane::State::default())),
        }
    }

    pub fn from_config(
        panes: Configuration<pane::State>,
        popout_windows: Vec<(Configuration<pane::State>, WindowSpec)>,
        layout_id: uuid::Uuid,
        market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
        downloaded_tickers: std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
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
            layout_id,
        }
    }

    pub fn load_layout(&mut self, _main_window: window::Id) -> Task<Message> {
        let mut open_popouts_tasks: Vec<Task<Message>> = vec![];
        let mut new_popout = Vec::new();
        let mut keys_to_remove = Vec::new();

        for (old_window_id, (_, specs)) in &self.popout {
            keys_to_remove.push((*old_window_id, specs.clone()));
        }

        // remove keys and open new windows
        for (old_window_id, window_spec) in keys_to_remove {
            let (pos_x, pos_y) = window_spec.clone().position_coords();
            let (width, height) = window_spec.clone().size_coords();

            let (window, task) = window::open(window::Settings {
                position: window::Position::Specific(iced::Point::new(pos_x, pos_y)),
                size: iced::Size::new(width, height),
                exit_on_close_request: false,
                ..window::settings()
            });

            open_popouts_tasks.push(task.then(|_| Task::none()));

            if let Some((removed_pane, specs)) = self.popout.remove(&old_window_id) {
                new_popout.push((window, (removed_pane, specs)));
            }
        }

        // assign new windows to old panes
        for (window, (pane, specs)) in new_popout {
            self.popout.insert(window, (pane, specs));
        }

        Task::batch(open_popouts_tasks)
    }

    pub fn update(
        &mut self,
        message: Message,
        main_window: &Window,
        _layout_id: &uuid::Uuid,
    ) -> (Task<Message>, Option<Event>) {
        match message {
            Message::SavePopoutSpecs(specs) => {
                for (window_id, new_spec) in specs {
                    if let Some((_, spec)) = self.popout.get_mut(&window_id) {
                        *spec = new_spec;
                    }
                }
            }
            Message::ErrorOccurred(pane_id, err) => match pane_id {
                Some(id) => {
                    if let Some(state) = self.get_mut_pane_state_by_uuid(main_window.id, id) {
                        state.loading_status = LoadingStatus::Ready;
                        state.notifications.push(Toast::error(err.to_string()));
                    }
                }
                _ => {
                    return (
                        Task::done(Message::Notification(Toast::error(err.to_string()))),
                        None,
                    );
                }
            },
            Message::Pane(window, message) => match message {
                pane::Message::PaneClicked(pane) => {
                    self.focus = Some((window, pane));
                }
                pane::Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                    self.panes.resize(split, ratio);
                }
                pane::Message::PaneDragged(event) => {
                    if let pane_grid::DragEvent::Dropped { pane, target } = event {
                        self.panes.drop(pane, target);
                    }
                }
                pane::Message::SplitPane(axis, pane) => {
                    let focus_pane = if let Some((new_pane, _)) =
                        self.panes.split(axis, pane, pane::State::new())
                    {
                        Some(new_pane)
                    } else {
                        None
                    };

                    if Some(focus_pane).is_some() {
                        self.focus = Some((window, focus_pane.unwrap()));
                    }
                }
                pane::Message::ClosePane(pane) => {
                    // Get pane UUID before closing to clean up chart state
                    if let Some(pane_state) = self.panes.get(pane) {
                        let uuid = pane_state.unique_id();
                        self.charts.remove(&uuid);
                        log::debug!("Cleaned up chart state for closed pane {}", uuid);
                    }

                    if let Some((_, sibling)) = self.panes.close(pane) {
                        self.focus = Some((window, sibling));
                    }
                }
                pane::Message::MaximizePane(pane) => {
                    self.panes.maximize(pane);
                }
                pane::Message::Restore => {
                    self.panes.restore();
                }
                pane::Message::ReplacePane(pane) => {
                    if let Some(pane) = self.panes.get_mut(pane) {
                        *pane = pane::State::new();
                    }
                }
                pane::Message::VisualConfigChanged(pane, cfg, to_sync) => {
                    if to_sync {
                        if let Some(state) = self.get_pane(main_window.id, window, pane) {
                            // Extract studies from heatmap content if present
                            let studies_cfg: Option<Vec<data::domain::chart_ui_types::heatmap::HeatmapStudy>> = match &state.content {
                                pane::Content::Heatmap { studies, .. } => Some(studies.clone()),
                                _ => None,
                            };
                            let clusters_cfg = match &state.content {
                                pane::Content::Kline {
                                    kind: data::chart::KlineChartKind::Footprint { clusters, .. },
                                    ..
                                } => Some(*clusters),
                                _ => None,
                            };

                            self.iter_all_panes_mut(main_window.id)
                                .for_each(|(_, _, state)| {
                                    let should_apply = match state.settings.visual_config {
                                        Some(ref current_cfg) => {
                                            std::mem::discriminant(current_cfg)
                                                == std::mem::discriminant(&cfg)
                                        }
                                        None => matches!(
                                            (&cfg, &state.content),
                                            (
                                                data::layout::pane::VisualConfig::Kline(_),
                                                pane::Content::Kline { .. }
                                            ) | (
                                                data::layout::pane::VisualConfig::Heatmap(_),
                                                pane::Content::Heatmap { .. }
                                            ) | (
                                                data::layout::pane::VisualConfig::TimeAndSales(_),
                                                pane::Content::TimeAndSales(_)
                                            ) | (
                                                data::layout::pane::VisualConfig::Comparison(_),
                                                pane::Content::Comparison(_)
                                            )
                                        ),
                                    };

                                    if should_apply {
                                        state.settings.visual_config = Some(cfg.clone());
                                        state.content.change_visual_config(cfg.clone());

                                        // Update studies for heatmap content
                                        if let Some(studies) = &studies_cfg {
                                            if let pane::Content::Heatmap { studies: hm_studies, .. } = &mut state.content {
                                                *hm_studies = studies.clone();
                                            }
                                        }

                                        if let Some(cluster_kind) = &clusters_cfg
                                            && let pane::Content::Kline { chart, .. } =
                                                &mut state.content
                                            && let Some(c) = chart
                                        {
                                            c.set_cluster_kind(*cluster_kind);
                                        }
                                    }
                                });
                        }
                    } else if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        state.settings.visual_config = Some(cfg.clone());
                        state.content.change_visual_config(cfg);
                    }
                }
                pane::Message::SwitchLinkGroup(pane, group) => {
                    if group.is_none() {
                        if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                            state.link_group = None;
                        }
                        return (Task::none(), None);
                    }

                    let _maybe_ticker_info = self
                        .iter_all_panes(main_window.id)
                        .filter(|(w, p, _)| !(*w == window && *p == pane))
                        .find_map(|(_, _, other_state)| {
                            if other_state.link_group == group {
                                other_state.ticker_info
                            } else {
                                None
                            }
                        });

                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        state.link_group = group;
                        state.modal = None;

                        // TODO: Handle ticker switching in link groups
                        // For now, just set the link group without loading data
                        // Will implement proper chart loading once pane.rs is refactored
                    }
                }
                pane::Message::Popout => {
                    return (self.popout_pane(main_window), None);
                }
                pane::Message::Merge => {
                    return (self.merge_pane(main_window), None);
                }
                pane::Message::PaneEvent(pane, local) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        let Some(effect) = state.update(local) else {
                            return (Task::none(), None);
                        };

                        // Handle pane effects
                        let pane_id = state.unique_id();
                        let triggering_pane_link_group = state.link_group; // Capture link group BEFORE matching on effect
                        let (task, event) = match effect {
                            pane::Effect::LoadChart { config, ticker_info } => {
                                // Trigger chart loading for this pane
                                let event = self.load_chart(pane_id, config, ticker_info);
                                (Task::none(), Some(event))
                            }
                            pane::Effect::SwitchTickersInGroup(ticker_info) => {
                                // Switch tickers for all panes in the same link group
                                // If no link group, pass pane_id to switch just this single pane
                                let task = self.switch_tickers_in_group(main_window.id, ticker_info, triggering_pane_link_group, Some(pane_id));
                                (task, None)
                            }
                            pane::Effect::FocusWidget(_id) => {
                                // TODO: Implement widget focusing with the specific ID
                                // For now, this effect is not critical for core functionality
                                (Task::none(), None)
                            }
                            pane::Effect::EstimateDataCost { ticker, schema, date_range } => {
                                // Trigger cost estimation
                                let task = Task::done(Message::EstimateDataCost {
                                    pane_id,
                                    ticker,
                                    schema,
                                    date_range,
                                });
                                (task, None)
                            }
                            pane::Effect::DownloadData { ticker, schema, date_range } => {
                                // Trigger data download
                                let task = Task::done(Message::DownloadData {
                                    pane_id,
                                    ticker,
                                    schema,
                                    date_range,
                                });
                                (task, None)
                            }
                        };
                        return (task, event);
                    }
                }
            },
            Message::ChangePaneStatus(pane_id, status) => {
                if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window.id, pane_id) {
                    pane_state.loading_status = status;
                }
            }
            Message::ChartDataLoaded { pane_id, chart_data } => {
                return (
                    self.handle_chart_data_loaded(main_window.id, pane_id, chart_data),
                    None,
                );
            }
            Message::LoadChart { pane_id, config, ticker_info } => {
                let event = self.load_chart(pane_id, config, ticker_info);
                return (Task::none(), Some(event));
            }
            Message::Notification(toast) => {
                return (Task::none(), Some(Event::Notification(toast)));
            }
            Message::EstimateDataCost { pane_id, ticker, schema, date_range } => {
                // This message should be forwarded to main - return as Event
                return (
                    Task::none(),
                    Some(Event::EstimateDataCost { pane_id, ticker, schema, date_range }),
                );
            }
            Message::DataCostEstimated { pane_id, total_days: _, cached_days: _, uncached_days: _, gaps_desc: _, actual_cost_usd: _, cached_dates: _ } => {
                // This message variant is deprecated - pane modals shouldn't use data management
                // Data management is now sidebar-only
                log::warn!("DataCostEstimated for pane {} - ignoring (sidebar-only feature)", pane_id);
            }
            Message::DownloadData { pane_id, ticker, schema, date_range } => {
                // This message should be forwarded to main - return as Event
                return (
                    Task::none(),
                    Some(Event::DownloadData { pane_id, ticker, schema, date_range }),
                );
            }
            Message::DataDownloadProgress { pane_id, current, total } => {
                // Update progress in data management modal
                if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window.id, pane_id) {
                    if let Some(Modal::DataManagement(ref mut panel)) = pane_state.modal {
                        panel.set_download_progress(DownloadProgress::Downloading {
                            current_day: current,
                            total_days: total,
                        });
                    }
                }
            }
            Message::DataDownloadComplete { pane_id, days_downloaded } => {
                // Mark download as complete in modal
                if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window.id, pane_id) {
                    if let Some(Modal::DataManagement(ref mut panel)) = pane_state.modal {
                        panel.set_download_progress(DownloadProgress::Complete {
                            days_downloaded,
                        });
                    }
                }
            }
        }

        (Task::none(), None)
    }

    fn new_pane(
        &mut self,
        axis: pane_grid::Axis,
        main_window: &Window,
        pane_state: Option<pane::State>,
    ) -> Task<Message> {
        if self
            .focus
            .filter(|(window, _)| *window == main_window.id)
            .is_some()
        {
            // If there is any focused pane on main window, split it
            return self.split_pane(axis, main_window);
        } else {
            // If there is no focused pane, split the last pane or create a new empty grid
            let pane = self.panes.iter().last().map(|(pane, _)| pane).copied();

            if let Some(pane) = pane {
                let result = self.panes.split(axis, pane, pane_state.unwrap_or_default());

                if let Some((pane, _)) = result {
                    return self.focus_pane(main_window.id, pane);
                }
            } else {
                let (state, pane) = pane_grid::State::new(pane_state.unwrap_or_default());
                self.panes = state;

                return self.focus_pane(main_window.id, pane);
            }
        }

        Task::none()
    }

    fn focus_pane(&mut self, window: window::Id, pane: pane_grid::Pane) -> Task<Message> {
        if self.focus != Some((window, pane)) {
            self.focus = Some((window, pane));
        }

        Task::none()
    }

    fn split_pane(&mut self, axis: pane_grid::Axis, main_window: &Window) -> Task<Message> {
        if let Some((window, pane)) = self.focus
            && window == main_window.id
        {
            let result = self.panes.split(axis, pane, pane::State::new());

            if let Some((pane, _)) = result {
                return self.focus_pane(main_window.id, pane);
            }
        }

        Task::none()
    }

    fn popout_pane(&mut self, main_window: &Window) -> Task<Message> {
        if let Some((_, id)) = self.focus.take()
            && let Some((pane, _)) = self.panes.close(id)
        {
            let (window, task) = window::open(window::Settings {
                position: main_window
                    .position
                    .map(|point| window::Position::Specific(point + Vector::new(20.0, 20.0)))
                    .unwrap_or_default(),
                exit_on_close_request: false,
                min_size: Some(iced::Size::new(400.0, 300.0)),
                ..window::settings()
            });

            let (state, id) = pane_grid::State::new(pane);
            self.popout.insert(window, (state, WindowSpec::default()));

            return task.then(move |window| {
                Task::done(Message::Pane(window, pane::Message::PaneClicked(id)))
            });
        }

        Task::none()
    }

    fn merge_pane(&mut self, main_window: &Window) -> Task<Message> {
        if let Some((window, pane)) = self.focus.take()
            && let Some(pane_state) = self
                .popout
                .remove(&window)
                .and_then(|(mut panes, _)| panes.panes.remove(&pane))
        {
            let task = self.new_pane(pane_grid::Axis::Horizontal, main_window, Some(pane_state));

            return Task::batch(vec![window::close(window), task]);
        }

        Task::none()
    }

    pub fn get_pane(
        &self,
        main_window: window::Id,
        window: window::Id,
        pane: pane_grid::Pane,
    ) -> Option<&pane::State> {
        if main_window == window {
            self.panes.get(pane)
        } else {
            self.popout
                .get(&window)
                .and_then(|(panes, _)| panes.get(pane))
        }
    }

    fn get_mut_pane(
        &mut self,
        main_window: window::Id,
        window: window::Id,
        pane: pane_grid::Pane,
    ) -> Option<&mut pane::State> {
        if main_window == window {
            self.panes.get_mut(pane)
        } else {
            self.popout
                .get_mut(&window)
                .and_then(|(panes, _)| panes.get_mut(pane))
        }
    }

    fn get_mut_pane_state_by_uuid(
        &mut self,
        main_window: window::Id,
        uuid: uuid::Uuid,
    ) -> Option<&mut pane::State> {
        self.iter_all_panes_mut(main_window)
            .find(|(_, _, state)| state.unique_id() == uuid)
            .map(|(_, _, state)| state)
    }

    fn iter_all_panes(
        &self,
        main_window: window::Id,
    ) -> impl Iterator<Item = (window::Id, pane_grid::Pane, &pane::State)> {
        self.panes
            .iter()
            .map(move |(pane, state)| (main_window, *pane, state))
            .chain(self.popout.iter().flat_map(|(window_id, (panes, _))| {
                panes.iter().map(|(pane, state)| (*window_id, *pane, state))
            }))
    }

    fn iter_all_panes_mut(
        &mut self,
        main_window: window::Id,
    ) -> impl Iterator<Item = (window::Id, pane_grid::Pane, &mut pane::State)> {
        self.panes
            .iter_mut()
            .map(move |(pane, state)| (main_window, *pane, state))
            .chain(self.popout.iter_mut().flat_map(|(window_id, (panes, _))| {
                panes
                    .iter_mut()
                    .map(|(pane, state)| (*window_id, *pane, state))
            }))
    }

    pub fn view<'a>(
        &'a self,
        main_window: &'a Window,
        tickers_table: &'a TickersTable,
        timezone: UserTimezone,
    ) -> Element<'a, Message> {
        let pane_grid: Element<_> = PaneGrid::new(&self.panes, |id, pane, maximized| {
            let is_focused = self.focus == Some((main_window.id, id));
            pane.view(
                id,
                self.panes.len(),
                is_focused,
                maximized,
                main_window.id,
                main_window,
                timezone,
                tickers_table,
            )
        })
        .min_size(240)
        .on_click(pane::Message::PaneClicked)
        .on_drag(pane::Message::PaneDragged)
        .on_resize(8, pane::Message::PaneResized)
        .spacing(6)
        .style(style::pane_grid)
        .into();

        pane_grid.map(move |message| Message::Pane(main_window.id, message))
    }

    pub fn view_window<'a>(
        &'a self,
        window: window::Id,
        main_window: &'a Window,
        tickers_table: &'a TickersTable,
        timezone: UserTimezone,
    ) -> Element<'a, Message> {
        if let Some((state, _)) = self.popout.get(&window) {
            let content = container(
                PaneGrid::new(state, |id, pane, _maximized| {
                    let is_focused = self.focus == Some((window, id));
                    pane.view(
                        id,
                        state.len(),
                        is_focused,
                        false,
                        window,
                        main_window,
                        timezone,
                        tickers_table,
                    )
                })
                .on_click(pane::Message::PaneClicked),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8);

            Element::new(content).map(move |message| Message::Pane(window, message))
        } else {
            Element::new(center("No pane found for window"))
                .map(move |message| Message::Pane(window, message))
        }
    }

    pub fn go_back(&mut self, main_window: window::Id) -> bool {
        let Some((window, pane)) = self.focus else {
            return false;
        };

        let Some(state) = self.get_mut_pane(main_window, window, pane) else {
            return false;
        };

        if state.modal.is_some() {
            state.modal = None;
            return true;
        }
        false
    }

    fn handle_error(
        &mut self,
        pane_id: Option<uuid::Uuid>,
        err: &DashboardError,
        main_window: window::Id,
    ) -> Task<Message> {
        match pane_id {
            Some(id) => {
                if let Some(state) = self.get_mut_pane_state_by_uuid(main_window, id) {
                    state.loading_status = LoadingStatus::Ready;
                    state.notifications.push(Toast::error(err.to_string()));
                }
                Task::none()
            }
            _ => Task::done(Message::Notification(Toast::error(err.to_string()))),
        }
    }


    pub fn init_focused_pane(
        &mut self,
        main_window: window::Id,
        ticker_info: FuturesTickerInfo,
        content_kind: ContentKind,
    ) -> Task<Message> {
        log::info!("DASHBOARD: init_focused_pane called with {:?} ContentKind::{:?}", ticker_info.ticker, content_kind);

        // Get the focused pane
        let Some((window, pane)) = self.focus else {
            log::warn!("No pane focused when trying to initialize");
            return Task::done(Message::Notification(Toast::warn(
                "No pane selected".to_string(),
            )));
        };

        // Get registered date range BEFORE borrowing pane_state mutably
        let date_range = self.downloaded_tickers
            .lock()
            .unwrap()
            .get_range(&ticker_info.ticker)
            .unwrap_or_else(|| {
                log::warn!("No registered range for {} - using last 1 day", ticker_info.ticker);
                DateRange::last_n_days(1)
            });

        log::info!("DASHBOARD: Using date range {} to {} for {}", date_range.start, date_range.end, ticker_info.ticker);

        // Get mutable reference to the focused pane state
        let Some(pane_state) = self.get_mut_pane(main_window, window, pane) else {
            log::error!("Focused pane not found in state");
            return Task::done(Message::Notification(Toast::error(
                "Failed to find pane".to_string(),
            )));
        };

        // Set content and trigger chart loading with registered date range
        let effect = pane_state.set_content_with_range(ticker_info, content_kind, date_range);

        // Handle the LoadChart effect
        match effect {
            pane::Effect::LoadChart { config, ticker_info } => {
                let pane_id = pane_state.unique_id();
                let event = self.load_chart(pane_id, config, ticker_info);

                // Return task that will emit the LoadChart event
                match event {
                    Event::LoadChart { pane_id, config, ticker_info } => {
                        Task::done(Message::LoadChart { pane_id, config, ticker_info })
                    }
                    Event::Notification(toast) => {
                        Task::done(Message::Notification(toast))
                    }
                    Event::EstimateDataCost { .. } | Event::DownloadData { .. } => {
                        // These shouldn't appear from set_content, but handle gracefully
                        Task::none()
                    }
                }
            }
            _ => {
                log::warn!("Unexpected effect from set_content: {:?}", effect);
                Task::none()
            }
        }
    }

    pub fn switch_tickers_in_group(
        &mut self,
        main_window: window::Id,
        ticker_info: FuturesTickerInfo,
        triggering_pane_link_group: Option<LinkGroup>,
        fallback_pane_id: Option<uuid::Uuid>, // NEW: If no link group, use this single pane
    ) -> Task<Message> {
        let mut panes_to_update = Vec::new();

        // If pane has a link group, update ALL panes in that group
        if let Some(link_group) = triggering_pane_link_group {
            log::info!("Switching tickers in link group {:?} to {:?}", link_group, ticker_info.ticker);

            // Collect all panes in this link group
            for (window, pane, state) in self.iter_all_panes(main_window) {
                if state.link_group == Some(link_group) {
                    panes_to_update.push((window, pane, state.unique_id(), state.content.kind()));
                }
            }
        } else if let Some(pane_id) = fallback_pane_id {
            // No link group - just update the single triggering pane
            log::info!("No link group - switching single pane {} to {:?}", pane_id, ticker_info.ticker);

            // Find the pane by UUID
            if let Some((window, pane, state)) = self.iter_all_panes(main_window)
                .find(|(_, _, s)| s.unique_id() == pane_id)
            {
                panes_to_update.push((window, pane, state.unique_id(), state.content.kind()));
            } else {
                log::error!("Could not find triggering pane by UUID: {}", pane_id);
                return Task::none();
            }
        } else {
            log::debug!("No link group and no fallback pane ID - cannot switch tickers");
            return Task::none();
        }

        log::info!("Switching {} pane(s) to ticker {:?}", panes_to_update.len(), ticker_info.ticker);

        // Get registered date range BEFORE looping
        let date_range = self.downloaded_tickers
            .lock()
            .unwrap()
            .get_range(&ticker_info.ticker)
            .unwrap_or_else(|| {
                log::warn!("No registered range for {} in switch_tickers_in_group - using last 1 day", ticker_info.ticker);
                DateRange::last_n_days(1)
            });

        log::info!("Using date range {} to {} for ticker switch", date_range.start, date_range.end);

        // Update each pane's ticker and trigger reload
        let mut tasks = Vec::new();
        for (_, _, pane_id, content_kind) in panes_to_update {
            // Get the pane state and update it
            if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window, pane_id) {
                // Use set_content_with_range to use registered date range (not default 1 day)
                let effect = pane_state.set_content_with_range(ticker_info, content_kind, date_range);

                log::info!("  Pane {} effect received: {:?}", pane_id,
                    match &effect {
                        pane::Effect::LoadChart { config, .. } => format!("LoadChart({:?})", config.chart_type),
                        pane::Effect::SwitchTickersInGroup(_) => "SwitchTickersInGroup".to_string(),
                        _ => "Other".to_string()
                    });

                // Handle the LoadChart effect
                if let pane::Effect::LoadChart { config, ticker_info } = effect {
                    log::info!("  Creating LoadChart event for pane {}", pane_id);
                    let event = self.load_chart(pane_id, config, ticker_info);
                    if let Event::LoadChart { pane_id, config, ticker_info } = event {
                        log::info!("  Pushing LoadChart message to task queue");
                        tasks.push(Message::LoadChart { pane_id, config, ticker_info });
                    }
                }
            }
        }

        // Return task that triggers all chart loads
        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks.into_iter().map(Task::done))
        }
    }


    /// Handle loaded chart data for a specific pane
    pub fn handle_chart_data_loaded(
        &mut self,
        main_window: window::Id,
        pane_id: uuid::Uuid,
        chart_data: ChartData,
    ) -> Task<Message> {
        // Update chart state first (separate borrow)
        if let Some(chart_state) = self.charts.get_mut(&pane_id) {
            chart_state.data = chart_data.clone();
            chart_state.loading_status = LoadingStatus::Ready;
        }

        // Update pane state (separate borrow)
        if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window, pane_id) {
            pane_state.set_chart_data(chart_data);
            log::info!("Chart data loaded for pane {}", pane_id);
        } else {
            log::warn!("Pane {} not found for chart data", pane_id);
        }

        Task::none()
    }

    /// Request chart data loading for a pane
    pub fn load_chart(
        &mut self,
        pane_id: uuid::Uuid,
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    ) -> Event {
        // Update loading status
        if let Some(chart_state) = self.charts.get_mut(&pane_id) {
            chart_state.loading_status = LoadingStatus::Building { operation: String::new(), progress: 0.0 };
        } else {
            // Create new chart state
            self.charts.insert(
                pane_id,
                ChartState {
                    config: config.clone(),
                    data: ChartData::from_trades(vec![], vec![]),
                    ticker_info,
                    loading_status: LoadingStatus::Building { operation: String::new(), progress: 0.0 },
                },
            );
        }

        Event::LoadChart {
            pane_id,
            config,
            ticker_info,
        }
    }


    pub fn invalidate_all_panes(&mut self, main_window: window::Id) {
        self.iter_all_panes_mut(main_window)
            .for_each(|(_, _, state)| {
                let _ = state.invalidate(Instant::now());
            });
    }

    pub fn tick(&mut self, now: Instant, main_window: window::Id) -> Task<Message> {
        // Tick all panes for canvas invalidation and animations
        self.iter_all_panes_mut(main_window)
            .for_each(|(_window_id, _pane, state)| {
                // Just invalidate charts for rendering updates
                let _ = state.invalidate(now);

                // TODO: Handle pane actions once pane.rs is refactored
                // For now, we skip action handling as we're moving to async loading model
            });

        Task::none()
    }


}
