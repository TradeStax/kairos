//! Backtest Management Modal
//!
//! Central coordinator for viewing backtest results: sidebar with
//! run history, tabbed main area (overview, trades, analytics),
//! and footer actions (delete, export CSV).

pub mod analytics;
pub mod charts;
pub mod computed;
pub mod overview;
pub mod sidebar;
pub mod trade_detail;
pub mod trades;

use crate::app::backtest_history::{BacktestHistory, BacktestStatus};
use crate::components::overlay::modal_header::ModalHeaderBuilder;
use crate::components::primitives::icon_button::icon_button;
use crate::components::primitives::icons::Icon;
use crate::config::UserTimezone;
use crate::style::{self, tokens};
use iced::widget::{button, canvas, column, container, row, rule, text};
use iced::{Element, Length};

// ── Tab Enum ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerTab {
    Overview,
    Trades,
    Analytics,
}

impl ManagerTab {
    pub const ALL: &'static [ManagerTab] = &[
        ManagerTab::Overview,
        ManagerTab::Trades,
        ManagerTab::Analytics,
    ];
}

impl std::fmt::Display for ManagerTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManagerTab::Overview => write!(f, "Overview"),
            ManagerTab::Trades => write!(f, "Trades"),
            ManagerTab::Analytics => write!(f, "Analytics"),
        }
    }
}

// ── Trade List Sort Column ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeListSortColumn {
    Index,
    EntryTime,
    ExitTime,
    Side,
    PnlUsd,
    PnlTicks,
    RrRatio,
    ExitReason,
}

// ── Messages ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ManagerMessage {
    SelectBacktest(uuid::Uuid),
    ChangeTab(ManagerTab),
    SortTrades(TradeListSortColumn),
    SelectTrade(Option<usize>),
    SelectPropFirm(usize),
    ClosePropFirmDetail,
    NewBacktest,
    DeleteBacktest(uuid::Uuid),
    ExportCsv,
    ExportJson,
    Close,
    // Trade detail (also triggered internally from SelectTrade double-click)
    #[allow(dead_code)]
    ViewTradeDetail(usize),
    CloseTradeDetail,
}

// ── Actions ─────────────────────────────────────────────────────────

pub enum ManagerAction {
    None,
    OpenLaunchModal,
    DeleteBacktest(uuid::Uuid),
    ExportCsv(uuid::Uuid),
    ExportJson(uuid::Uuid),
    Close,
}

// ── Trade detail view state ─────────────────────────────────────────

pub struct TradeDetailView {
    pub trade_index: usize,
    pub chart_cache: canvas::Cache,
}

impl TradeDetailView {
    pub fn new(trade_index: usize) -> Self {
        Self {
            trade_index,
            chart_cache: canvas::Cache::new(),
        }
    }
}

// ── Prop Firm Detail View ───────────────────────────────────────────

pub struct PropFirmDetailView {
    pub account_index: usize,
    pub equity_chart_cache: canvas::Cache,
    pub mc_chart_cache: canvas::Cache,
}

impl PropFirmDetailView {
    pub fn new(account_index: usize) -> Self {
        Self {
            account_index,
            equity_chart_cache: canvas::Cache::new(),
            mc_chart_cache: canvas::Cache::new(),
        }
    }
}

// ── State ───────────────────────────────────────────────────────────

pub struct BacktestManager {
    pub selected_id: Option<uuid::Uuid>,
    pub active_tab: ManagerTab,
    // Trades tab
    pub sort_column: TradeListSortColumn,
    pub sort_ascending: bool,
    pub sorted_indices: Vec<usize>,
    pub selected_trade: Option<usize>,
    // Prop firm detail (full-page)
    pub prop_firm_detail: Option<PropFirmDetailView>,
    // Computed analytics (cached)
    pub analytics: Option<computed::ComputedAnalytics>,
    // Canvas caches
    pub equity_cache: canvas::Cache,
    pub drawdown_cache: canvas::Cache,
    pub monte_carlo_cache: canvas::Cache,
    pub histogram_cache: canvas::Cache,
    pub scatter_cache: canvas::Cache,
    pub bar_chart_cache: canvas::Cache,
    pub returns_cache: canvas::Cache,
    // Trade detail
    pub trade_detail: Option<TradeDetailView>,
}

impl BacktestManager {
    pub fn new() -> Self {
        Self {
            selected_id: None,
            active_tab: ManagerTab::Overview,
            sort_column: TradeListSortColumn::Index,
            sort_ascending: true,
            sorted_indices: Vec::new(),
            selected_trade: None,
            prop_firm_detail: None,
            analytics: None,
            equity_cache: canvas::Cache::new(),
            drawdown_cache: canvas::Cache::new(),
            monte_carlo_cache: canvas::Cache::new(),
            histogram_cache: canvas::Cache::new(),
            scatter_cache: canvas::Cache::new(),
            bar_chart_cache: canvas::Cache::new(),
            returns_cache: canvas::Cache::new(),
            trade_detail: None,
        }
    }

    /// Programmatically select a backtest (used by
    /// app-level handlers).
    pub fn select(&mut self, id: uuid::Uuid, history: &BacktestHistory, timezone: UserTimezone) {
        self.update(ManagerMessage::SelectBacktest(id), history, timezone);
    }

    pub fn update(
        &mut self,
        message: ManagerMessage,
        history: &BacktestHistory,
        timezone: UserTimezone,
    ) -> ManagerAction {
        match message {
            ManagerMessage::SelectBacktest(id) => {
                self.selected_id = Some(id);
                self.active_tab = ManagerTab::Overview;
                self.selected_trade = None;
                self.prop_firm_detail = None;
                self.trade_detail = None;

                if let Some(entry) = history.get(id) {
                    if entry.status == BacktestStatus::Completed {
                        if let Some(ref result) = entry.result {
                            self.analytics =
                                Some(computed::ComputedAnalytics::from_result(result, timezone));
                            let n = result.trades.len();
                            self.sorted_indices = (0..n).collect();
                            self.sort_column = TradeListSortColumn::Index;
                            self.sort_ascending = true;
                        }
                    } else {
                        self.analytics = None;
                        self.sorted_indices.clear();
                    }
                } else {
                    self.analytics = None;
                    self.sorted_indices.clear();
                }

                self.clear_all_caches();
                ManagerAction::None
            }
            ManagerMessage::ChangeTab(tab) => {
                self.active_tab = tab;
                ManagerAction::None
            }
            ManagerMessage::SortTrades(col) => {
                if self.sort_column == col {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = col;
                    self.sort_ascending = true;
                }
                self.resort_trades(history);
                ManagerAction::None
            }
            ManagerMessage::SelectTrade(idx) => {
                // Click-to-select, click-again-to-open pattern:
                // if the same trade is already selected, open detail view
                if let Some(i) = idx
                    && self.selected_trade == Some(i)
                {
                    self.trade_detail = Some(TradeDetailView::new(i));
                    return ManagerAction::None;
                }
                self.selected_trade = idx;
                self.equity_cache.clear();
                ManagerAction::None
            }
            ManagerMessage::SelectPropFirm(idx) => {
                self.prop_firm_detail = Some(PropFirmDetailView::new(idx));
                ManagerAction::None
            }
            ManagerMessage::ClosePropFirmDetail => {
                self.prop_firm_detail = None;
                ManagerAction::None
            }
            ManagerMessage::NewBacktest => ManagerAction::OpenLaunchModal,
            ManagerMessage::DeleteBacktest(id) => ManagerAction::DeleteBacktest(id),
            ManagerMessage::ExportCsv => {
                if let Some(id) = self.selected_id {
                    ManagerAction::ExportCsv(id)
                } else {
                    ManagerAction::None
                }
            }
            ManagerMessage::ExportJson => {
                if let Some(id) = self.selected_id {
                    ManagerAction::ExportJson(id)
                } else {
                    ManagerAction::None
                }
            }
            ManagerMessage::Close => ManagerAction::Close,
            // Trade detail messages
            ManagerMessage::ViewTradeDetail(idx) => {
                self.trade_detail = Some(TradeDetailView::new(idx));
                ManagerAction::None
            }
            ManagerMessage::CloseTradeDetail => {
                self.trade_detail = None;
                ManagerAction::None
            }
        }
    }

    pub fn resort_trades(&mut self, history: &BacktestHistory) {
        let Some(id) = self.selected_id else {
            return;
        };
        let Some(entry) = history.get(id) else {
            return;
        };
        let Some(ref result) = entry.result else {
            return;
        };
        let trades = &result.trades;
        let ascending = self.sort_ascending;

        self.sorted_indices.sort_by(|&a, &b| {
            let cmp = match self.sort_column {
                TradeListSortColumn::Index => trades[a].index.cmp(&trades[b].index),
                TradeListSortColumn::EntryTime => trades[a].entry_time.cmp(&trades[b].entry_time),
                TradeListSortColumn::ExitTime => trades[a].exit_time.cmp(&trades[b].exit_time),
                TradeListSortColumn::Side => {
                    format!("{:?}", trades[a].side).cmp(&format!("{:?}", trades[b].side))
                }
                TradeListSortColumn::PnlUsd => trades[a]
                    .pnl_net_usd
                    .partial_cmp(&trades[b].pnl_net_usd)
                    .unwrap_or(std::cmp::Ordering::Equal),
                TradeListSortColumn::PnlTicks => trades[a].pnl_ticks.cmp(&trades[b].pnl_ticks),
                TradeListSortColumn::RrRatio => trades[a]
                    .rr_ratio
                    .partial_cmp(&trades[b].rr_ratio)
                    .unwrap_or(std::cmp::Ordering::Equal),
                TradeListSortColumn::ExitReason => trades[a]
                    .exit_reason
                    .to_string()
                    .cmp(&trades[b].exit_reason.to_string()),
            };
            if ascending { cmp } else { cmp.reverse() }
        });
    }

    pub fn clear_all_caches(&mut self) {
        self.equity_cache.clear();
        self.drawdown_cache.clear();
        self.monte_carlo_cache.clear();
        self.histogram_cache.clear();
        self.scatter_cache.clear();
        self.bar_chart_cache.clear();
        self.returns_cache.clear();
        if let Some(ref detail) = self.prop_firm_detail {
            detail.equity_chart_cache.clear();
            detail.mc_chart_cache.clear();
        }
    }

    /// Whether the currently selected backtest is completed with
    /// a result available.
    fn selected_is_completed(&self, history: &BacktestHistory) -> bool {
        self.selected_id
            .and_then(|id| history.get(id))
            .map(|e| e.status == BacktestStatus::Completed && e.result.is_some())
            .unwrap_or(false)
    }

    pub fn view<'a>(
        &'a self,
        history: &'a BacktestHistory,
        timezone: UserTimezone,
    ) -> Element<'a, ManagerMessage> {
        let header = ModalHeaderBuilder::new("Backtest Manager")
            .on_close(ManagerMessage::Close)
            .into_element();

        // ── Sidebar ─────────────────────────────────────────────
        let sidebar_content = sidebar::view_sidebar(self, history);
        let sidebar_col = container(sidebar_content)
            .width(Length::Fixed(210.0))
            .height(Length::Fill);

        // ── Main content ────────────────────────────────────────
        let main_area = self.view_main_area(history, timezone);

        let body = row![
            sidebar_col,
            rule::vertical(1),
            container(main_area)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(tokens::spacing::LG),
        ]
        .height(Length::Fill);

        // ── Footer ──────────────────────────────────────────────
        let footer = self.view_footer(history);

        let mut content = column![header, rule::horizontal(1), body]
            .width(Length::Fill)
            .height(Length::Fill);
        if let Some(f) = footer {
            content = content.push(f);
        }

        container(content)
            .max_width(1100)
            .max_height(800)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(style::dashboard_modal)
            .into()
    }

    fn view_main_area<'a>(
        &'a self,
        history: &'a BacktestHistory,
        timezone: UserTimezone,
    ) -> Element<'a, ManagerMessage> {
        let Some(id) = self.selected_id else {
            return Self::view_empty_state();
        };
        let Some(entry) = history.get(id) else {
            return Self::view_empty_state();
        };

        if entry.status != BacktestStatus::Completed || entry.result.is_none() {
            return Self::view_pending_state(entry);
        }

        // Tab bar
        let tab_bar = self.view_tab_bar();

        // Tab content — trade detail replaces trades table when active
        let tab_content = match self.active_tab {
            ManagerTab::Overview => overview::view_overview(self, history, timezone),
            ManagerTab::Trades => {
                if let Some(ref detail) = self.trade_detail {
                    if let Some(ref result) = entry.result {
                        trade_detail::view(detail, result, &result.config, timezone)
                    } else {
                        trades::view(self, history, timezone)
                    }
                } else {
                    trades::view(self, history, timezone)
                }
            }
            ManagerTab::Analytics => analytics::view(self, history, timezone),
        };

        column![tab_bar, tab_content]
            .spacing(tokens::spacing::MD)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_tab_bar(&self) -> Element<'_, ManagerMessage> {
        let mut tab_row = row![].spacing(tokens::spacing::XS);

        for &tab in ManagerTab::ALL {
            let is_active = self.active_tab == tab;
            let label = text(tab.to_string()).size(tokens::text::LABEL);

            let btn = button(label)
                .padding([tokens::spacing::SM, tokens::spacing::LG])
                .on_press(ManagerMessage::ChangeTab(tab))
                .style(move |theme, status| {
                    if is_active {
                        style::button::tab_active(theme, status)
                    } else {
                        style::button::tab_inactive(theme, status)
                    }
                });

            tab_row = tab_row.push(btn);
        }

        tab_row.into()
    }

    fn view_footer<'a>(
        &'a self,
        history: &'a BacktestHistory,
    ) -> Option<Element<'a, ManagerMessage>> {
        if !self.selected_is_completed(history) {
            return None;
        }
        let id = self.selected_id?;

        let footer_padding = [tokens::spacing::SM, tokens::spacing::LG];

        let delete_btn = icon_button(Icon::TrashBin)
            .size(tokens::text::BODY)
            .tooltip("Delete")
            .on_press(ManagerMessage::DeleteBacktest(id))
            .style(style::button::danger);

        let export_csv_btn = button(text("Export CSV").size(tokens::text::BODY))
            .padding(footer_padding)
            .on_press(ManagerMessage::ExportCsv)
            .style(style::button::secondary);

        let export_json_btn = button(text("Export JSON").size(tokens::text::BODY))
            .padding(footer_padding)
            .on_press(ManagerMessage::ExportJson)
            .style(style::button::secondary);

        let footer_row = row![
            iced::widget::Space::new().width(Length::Fill),
            delete_btn,
            export_csv_btn,
            export_json_btn,
        ]
        .spacing(tokens::spacing::SM)
        .padding(tokens::spacing::LG)
        .align_y(iced::Alignment::Center);

        let footer = column![rule::horizontal(1), footer_row,];

        Some(footer.into())
    }

    fn view_empty_state<'a>() -> Element<'a, ManagerMessage> {
        let msg = text("Select a backtest from the sidebar")
            .size(tokens::text::LABEL)
            .color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.4));

        container(msg)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center)
            .into()
    }

    fn view_pending_state<'a>(
        entry: &crate::app::backtest_history::BacktestHistoryEntry,
    ) -> Element<'a, ManagerMessage> {
        let status_text = match entry.status {
            BacktestStatus::Running => {
                let pct = (entry.progress * 100.0) as u32;
                format!("Running... {}% - {}", pct, entry.progress_message,)
            }
            BacktestStatus::Failed => {
                let err = entry.error.as_deref().unwrap_or("Unknown error");
                format!("Failed: {}", err)
            }
            BacktestStatus::Completed => "Completed (no result data)".to_string(),
        };

        let label = text(status_text)
            .size(tokens::text::LABEL)
            .color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.5));

        container(label)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center)
            .into()
    }
}
