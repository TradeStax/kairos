//! Backtest Launch Modal
//!
//! Two-column modal for configuring and launching backtests.
//! Left panel: search, category filter, strategy list.
//! Right panel: strategy parameters, dataset, execution settings.

mod catalog_view;
mod settings_view;

use crate::modals::pane::calendar::{CalendarMessage, DateRangeCalendar};
use backtest::config::risk::{PositionSizeMode, RiskConfig};
use backtest::{BacktestConfig, SlippageModel, StrategyCategory, StrategyRegistry};
use chrono::{Datelike, NaiveDate};
use data::Timeframe;
use std::collections::{BTreeSet, HashMap, HashSet};

// ── Category Filter ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryFilter {
    All,
    BreakoutMomentum,
    MeanReversion,
    TrendFollowing,
    OrderFlow,
    Custom,
}

impl CategoryFilter {
    pub const ALL: &'static [CategoryFilter] = &[
        CategoryFilter::All,
        CategoryFilter::BreakoutMomentum,
        CategoryFilter::MeanReversion,
        CategoryFilter::TrendFollowing,
        CategoryFilter::OrderFlow,
        CategoryFilter::Custom,
    ];

    pub(super) fn matches(self, category: StrategyCategory) -> bool {
        match self {
            CategoryFilter::All => true,
            CategoryFilter::BreakoutMomentum => category == StrategyCategory::BreakoutMomentum,
            CategoryFilter::MeanReversion => category == StrategyCategory::MeanReversion,
            CategoryFilter::TrendFollowing => category == StrategyCategory::TrendFollowing,
            CategoryFilter::OrderFlow => category == StrategyCategory::OrderFlow,
            CategoryFilter::Custom => category == StrategyCategory::Custom,
        }
    }
}

impl std::fmt::Display for CategoryFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CategoryFilter::All => write!(f, "All"),
            CategoryFilter::BreakoutMomentum => write!(f, "Breakout"),
            CategoryFilter::MeanReversion => write!(f, "Mean Rev."),
            CategoryFilter::TrendFollowing => write!(f, "Trend"),
            CategoryFilter::OrderFlow => write!(f, "Order Flow"),
            CategoryFilter::Custom => write!(f, "Custom"),
        }
    }
}

// ── Settings Tab ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Parameters,
    Dataset,
    Execution,
}

impl SettingsTab {
    pub const ALL: &'static [SettingsTab] = &[
        SettingsTab::Parameters,
        SettingsTab::Dataset,
        SettingsTab::Execution,
    ];
}

impl std::fmt::Display for SettingsTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingsTab::Parameters => write!(f, "Parameters"),
            SettingsTab::Dataset => write!(f, "Dataset"),
            SettingsTab::Execution => write!(f, "Execution"),
        }
    }
}

// ── Slippage Mode ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlippageMode {
    None,
    FixedTick,
    Percentage,
}

impl std::fmt::Display for SlippageMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::FixedTick => write!(f, "Fixed Ticks"),
            Self::Percentage => write!(f, "Percentage"),
        }
    }
}

// ── Position Size Mode (UI) ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionSizeModeUI {
    Fixed,
    RiskPercent,
    RiskDollars,
}

impl std::fmt::Display for PositionSizeModeUI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fixed => write!(f, "Fixed Contracts"),
            Self::RiskPercent => write!(f, "Risk % of Equity"),
            Self::RiskDollars => write!(f, "Risk $ per Trade"),
        }
    }
}

// ── Calendar Mode ────────────────────────────────────────────────────

/// Controls calendar behavior based on connection type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CalendarMode {
    /// Databento: only cached dates are selectable
    CachedOnly,
    /// Rithmic: any date is selectable, data auto-downloads
    AnyDate,
}

// ── Connection Snapshot ──────────────────────────────────────────────

/// Pre-computed snapshot of a connection's data availability.
#[derive(Debug, Clone)]
pub(super) struct ConnectionSnapshot {
    #[allow(dead_code)]
    pub id: data::FeedId,
    pub name: String,
    pub provider: data::ConnectionProvider,
    pub tickers: Vec<(String, String)>,
    pub ticker_dates: HashMap<String, BTreeSet<NaiveDate>>,
    pub calendar_mode: CalendarMode,
}

impl std::fmt::Display for ConnectionSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.provider)
    }
}

// ── Messages ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    CategorySelected(CategoryFilter),
    SelectStrategy(String),
    TabChanged(SettingsTab),
    ParameterChanged {
        strategy_id: String,
        key: String,
        value: study::ParameterValue,
    },
    ConnectionSelected(usize),
    TickerSelected(String),
    TimeframeSelected(Timeframe),
    Calendar(CalendarMessage),
    InitialCapitalChanged(String),
    CommissionChanged(String),
    SlippageModeChanged(SlippageMode),
    SlippageTicksChanged(String),
    PositionSizeModeChanged(PositionSizeModeUI),
    PositionSizeValueChanged(String),
    MaxConcurrentChanged(String),
    MaxDrawdownToggled(bool),
    MaxDrawdownPctChanged(String),
    RthOpenChanged(String),
    RthCloseChanged(String),
    RunPressed,
    Close,
}

// ── Actions ──────────────────────────────────────────────────────────

pub enum Action {
    RunRequested(Box<BacktestConfig>),
    Closed,
}

// ── State ────────────────────────────────────────────────────────────

pub struct BacktestLaunchModal {
    // Left panel
    pub(super) search_query: String,
    pub(super) category_filter: CategoryFilter,
    pub(super) selected_strategy_id: Option<String>,

    // Strategy snapshots (created once from registry)
    pub(super) strategy_snapshots: Vec<(String, Box<dyn backtest::Strategy>)>,

    // Right panel
    pub(super) settings_tab: SettingsTab,

    // Dataset tab — connection-based selection
    pub(super) connections: Vec<ConnectionSnapshot>,
    pub(super) selected_connection_idx: Option<usize>,
    pub(super) connection_tickers: Vec<(String, String)>,
    pub(super) selected_ticker: Option<String>,
    pub(super) calendar_mode: CalendarMode,
    pub(super) calendar: DateRangeCalendar,

    // Execution tab (timeframe moved here from Dataset)
    pub(super) selected_timeframe: Timeframe,
    pub(super) initial_capital_str: String,
    pub(super) commission_str: String,
    pub(super) slippage_mode: SlippageMode,
    pub(super) slippage_ticks_str: String,
    pub(super) position_size_mode: PositionSizeModeUI,
    pub(super) position_size_value_str: String,
    pub(super) max_concurrent_str: String,
    pub(super) max_drawdown_enabled: bool,
    pub(super) max_drawdown_pct_str: String,
    pub(super) rth_open_str: String,
    pub(super) rth_close_str: String,

    // Run state
    pub(crate) is_running: bool,
    pub(crate) run_progress: f32,
    pub(crate) run_progress_message: String,
    pub(crate) validation_error: Option<String>,
}

impl std::fmt::Debug for BacktestLaunchModal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BacktestLaunchModal")
            .field("search_query", &self.search_query)
            .field("category_filter", &self.category_filter)
            .field("selected_strategy_id", &self.selected_strategy_id)
            .field("settings_tab", &self.settings_tab)
            .field("selected_ticker", &self.selected_ticker)
            .field("connections_count", &self.connections.len())
            .field("is_running", &self.is_running)
            .finish()
    }
}

impl Clone for BacktestLaunchModal {
    fn clone(&self) -> Self {
        Self {
            search_query: self.search_query.clone(),
            category_filter: self.category_filter,
            selected_strategy_id: self.selected_strategy_id.clone(),
            strategy_snapshots: self
                .strategy_snapshots
                .iter()
                .map(|(id, s)| (id.clone(), s.clone_strategy()))
                .collect(),
            settings_tab: self.settings_tab,
            connections: self.connections.clone(),
            selected_connection_idx: self.selected_connection_idx,
            connection_tickers: self.connection_tickers.clone(),
            selected_ticker: self.selected_ticker.clone(),
            calendar_mode: self.calendar_mode,
            calendar: self.calendar.clone(),
            selected_timeframe: self.selected_timeframe,
            initial_capital_str: self.initial_capital_str.clone(),
            commission_str: self.commission_str.clone(),
            slippage_mode: self.slippage_mode,
            slippage_ticks_str: self.slippage_ticks_str.clone(),
            position_size_mode: self.position_size_mode,
            position_size_value_str: self.position_size_value_str.clone(),
            max_concurrent_str: self.max_concurrent_str.clone(),
            max_drawdown_enabled: self.max_drawdown_enabled,
            max_drawdown_pct_str: self.max_drawdown_pct_str.clone(),
            rth_open_str: self.rth_open_str.clone(),
            rth_close_str: self.rth_close_str.clone(),
            is_running: self.is_running,
            run_progress: self.run_progress,
            run_progress_message: self.run_progress_message.clone(),
            validation_error: self.validation_error.clone(),
        }
    }
}

impl BacktestLaunchModal {
    pub fn new(
        registry: &StrategyRegistry,
        connection_manager: &data::ConnectionManager,
        data_index: &data::DataIndex,
    ) -> Self {
        let strategy_list = registry.list();
        let strategy_snapshots: Vec<(String, Box<dyn backtest::Strategy>)> = strategy_list
            .iter()
            .filter_map(|info| registry.create(&info.id).map(|s| (info.id.clone(), s)))
            .collect();

        let first_id = strategy_snapshots.first().map(|(id, _)| id.clone());

        // Build product name lookup from ticker registry
        let products = crate::app::init::ticker_registry::FUTURES_PRODUCTS;
        let product_names: HashMap<&str, &str> = products
            .iter()
            .map(|(sym, name, _, _, _)| (*sym, *name))
            .collect();

        // Build ConnectionSnapshot list from currently connected feeds
        let connections: Vec<ConnectionSnapshot> = connection_manager
            .enabled_connections()
            .into_iter()
            .filter(|conn| !matches!(conn.status, data::ConnectionStatus::Disconnected))
            .map(|conn| {
                let calendar_mode = match conn.provider {
                    data::ConnectionProvider::Databento => CalendarMode::CachedOnly,
                    data::ConnectionProvider::Rithmic => CalendarMode::AnyDate,
                };

                // Resolve tickers for this connection
                let mut raw_tickers: Vec<String> = match &conn.kind {
                    data::ConnectionKind::Historical(info) => {
                        vec![info.ticker.clone()]
                    }
                    data::ConnectionKind::Realtime => {
                        let mut tickers = data_index.tickers_for_feed(conn.id);
                        // For Rithmic, subscribed_tickers are bare product
                        // names ("NQ"), but tickers_for_feed returns
                        // continuous contract format ("NQ.c.0"). Resolve
                        // bare names to .c.0 before merging.
                        if let Some(cfg) = conn.rithmic_config() {
                            for bare in &cfg.subscribed_tickers {
                                let resolved = format!("{}.c.0", bare);
                                if !tickers.contains(&resolved) {
                                    tickers.push(resolved);
                                }
                            }
                        }
                        tickers
                    }
                };
                raw_tickers.sort();
                raw_tickers.dedup();

                // Map to display names
                let tickers: Vec<(String, String)> = raw_tickers
                    .iter()
                    .map(|sym| {
                        let name = product_names
                            .get(sym.as_str())
                            .copied()
                            .unwrap_or(sym.as_str());
                        (sym.clone(), name.to_string())
                    })
                    .collect();

                // Pre-compute per-ticker dates for this feed
                let ticker_dates: HashMap<String, BTreeSet<NaiveDate>> = raw_tickers
                    .iter()
                    .map(|sym| {
                        (
                            sym.clone(),
                            data_index.available_dates_for_feed(sym, conn.id),
                        )
                    })
                    .collect();

                ConnectionSnapshot {
                    id: conn.id,
                    name: conn.name.clone(),
                    provider: conn.provider,
                    tickers,
                    ticker_dates,
                    calendar_mode,
                }
            })
            .collect();

        // Auto-select first connection with tickers
        let auto_conn_idx = connections.iter().position(|c| !c.tickers.is_empty());

        let (connection_tickers, selected_ticker, calendar_mode, calendar) =
            if let Some(idx) = auto_conn_idx {
                let snap = &connections[idx];
                let tickers = snap.tickers.clone();
                let first_ticker = tickers.first().map(|(sym, _)| sym.clone());
                let mode = snap.calendar_mode;
                let mut cal = DateRangeCalendar::new();
                if let Some(ref sym) = first_ticker {
                    Self::configure_calendar(&mut cal, mode, snap.ticker_dates.get(sym));
                }
                (tickers, first_ticker, mode, cal)
            } else {
                (
                    Vec::new(),
                    None,
                    CalendarMode::CachedOnly,
                    DateRangeCalendar::new(),
                )
            };

        Self {
            search_query: String::new(),
            category_filter: CategoryFilter::All,
            selected_strategy_id: first_id,
            strategy_snapshots,
            settings_tab: SettingsTab::Parameters,
            connections,
            selected_connection_idx: auto_conn_idx,
            connection_tickers,
            selected_ticker,
            calendar_mode,
            calendar,
            selected_timeframe: Timeframe::M1,
            initial_capital_str: "100000".to_string(),
            commission_str: "2.50".to_string(),
            slippage_mode: SlippageMode::None,
            slippage_ticks_str: "0".to_string(),
            position_size_mode: PositionSizeModeUI::Fixed,
            position_size_value_str: "1".to_string(),
            max_concurrent_str: "1".to_string(),
            max_drawdown_enabled: false,
            max_drawdown_pct_str: "20".to_string(),
            rth_open_str: "930".to_string(),
            rth_close_str: "1600".to_string(),
            is_running: false,
            run_progress: 0.0,
            run_progress_message: String::new(),
            validation_error: None,
        }
    }

    /// Configure calendar based on mode and available dates.
    fn configure_calendar(
        calendar: &mut DateRangeCalendar,
        mode: CalendarMode,
        dates: Option<&BTreeSet<NaiveDate>>,
    ) {
        match mode {
            CalendarMode::CachedOnly => {
                if let Some(dates) = dates {
                    calendar.selectable_dates = Some(dates.iter().copied().collect::<HashSet<_>>());
                    if let (Some(&first), Some(&last)) =
                        (dates.iter().next(), dates.iter().next_back())
                    {
                        calendar.start_date = first;
                        calendar.end_date = last;
                        calendar.viewing_month =
                            NaiveDate::from_ymd_opt(first.year(), first.month(), 1).unwrap();
                    }
                } else {
                    calendar.selectable_dates = Some(HashSet::new());
                }
            }
            CalendarMode::AnyDate => {
                // All dates selectable, default to last week
                calendar.selectable_dates = None;
                if let Some(dates) = dates
                    && let (Some(&first), Some(&last)) =
                        (dates.iter().next(), dates.iter().next_back())
                {
                    calendar.start_date = first;
                    calendar.end_date = last;
                    calendar.viewing_month =
                        NaiveDate::from_ymd_opt(first.year(), first.month(), 1).unwrap();
                } else {
                    let range = data::DateRange::last_week();
                    calendar.start_date = range.start;
                    calendar.end_date = range.end;
                    calendar.viewing_month =
                        NaiveDate::from_ymd_opt(range.start.year(), range.start.month(), 1)
                            .unwrap();
                }
            }
        }
    }

    /// Apply a connection selection, updating tickers and calendar.
    fn apply_connection_selection(&mut self, idx: usize) {
        let Some(snap) = self.connections.get(idx) else {
            log::warn!("Connection index {} out of bounds", idx);
            return;
        };
        self.selected_connection_idx = Some(idx);
        self.connection_tickers = snap.tickers.clone();
        self.calendar_mode = snap.calendar_mode;

        // Auto-select first ticker
        self.selected_ticker = self.connection_tickers.first().map(|(sym, _)| sym.clone());

        // Configure calendar
        let dates = self
            .selected_ticker
            .as_ref()
            .and_then(|sym| snap.ticker_dates.get(sym));
        Self::configure_calendar(&mut self.calendar, self.calendar_mode, dates);
        self.calendar.selection_mode = crate::modals::pane::calendar::SelectionMode::SelectingStart;
    }

    pub fn set_running(&mut self, running: bool) {
        self.is_running = running;
        if !running {
            self.run_progress = 0.0;
        }
    }

    #[allow(dead_code)]
    pub fn set_progress(&mut self, pct: f32, message: String) {
        self.run_progress = pct;
        self.run_progress_message = message;
    }

    /// Pre-select a strategy by ID (from File -> New Backtest menu).
    pub fn pre_select_strategy(&mut self, strategy_id: &str) {
        if self
            .strategy_snapshots
            .iter()
            .any(|(id, _)| id == strategy_id)
        {
            self.selected_strategy_id = Some(strategy_id.to_string());
            self.settings_tab = SettingsTab::Parameters;
        }
    }

    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::SearchChanged(query) => {
                self.search_query = query;
            }
            Message::CategorySelected(cat) => {
                self.category_filter = cat;
            }
            Message::SelectStrategy(id) => {
                self.selected_strategy_id = Some(id);
                self.settings_tab = SettingsTab::Parameters;
            }
            Message::TabChanged(tab) => {
                self.settings_tab = tab;
            }
            Message::ParameterChanged {
                strategy_id,
                key,
                value,
            } => {
                if let Some((_, snapshot)) = self
                    .strategy_snapshots
                    .iter_mut()
                    .find(|(id, _)| id == &strategy_id)
                    && let Err(e) = snapshot.set_parameter(&key, value)
                {
                    log::warn!("Failed to set backtest param: {}", e);
                }
            }
            Message::ConnectionSelected(idx) => {
                if idx < self.connections.len() {
                    self.apply_connection_selection(idx);
                }
            }
            Message::TickerSelected(symbol) => {
                self.selected_ticker = Some(symbol.clone());
                // Reconfigure calendar for the new ticker
                if let Some(idx) = self.selected_connection_idx
                    && let Some(snap) = self.connections.get(idx)
                {
                    let dates = snap.ticker_dates.get(&symbol);
                    Self::configure_calendar(&mut self.calendar, self.calendar_mode, dates);
                }
                self.calendar.selection_mode =
                    crate::modals::pane::calendar::SelectionMode::SelectingStart;
            }
            Message::TimeframeSelected(tf) => {
                self.selected_timeframe = tf;
            }
            Message::Calendar(cal_msg) => {
                self.calendar.update(cal_msg);
            }
            Message::InitialCapitalChanged(s) => {
                self.initial_capital_str = s;
            }
            Message::CommissionChanged(s) => {
                self.commission_str = s;
            }
            Message::SlippageModeChanged(mode) => {
                self.slippage_mode = mode;
            }
            Message::SlippageTicksChanged(s) => {
                self.slippage_ticks_str = s;
            }
            Message::PositionSizeModeChanged(mode) => {
                self.position_size_mode = mode;
            }
            Message::PositionSizeValueChanged(s) => {
                self.position_size_value_str = s;
            }
            Message::MaxConcurrentChanged(s) => {
                self.max_concurrent_str = s;
            }
            Message::MaxDrawdownToggled(on) => {
                self.max_drawdown_enabled = on;
            }
            Message::MaxDrawdownPctChanged(s) => {
                self.max_drawdown_pct_str = s;
            }
            Message::RthOpenChanged(s) => {
                self.rth_open_str = s;
            }
            Message::RthCloseChanged(s) => {
                self.rth_close_str = s;
            }
            Message::RunPressed => {
                return self.build_and_validate_config();
            }
            Message::Close => {
                return Some(Action::Closed);
            }
        }
        None
    }

    fn build_and_validate_config(&mut self) -> Option<Action> {
        let initial_capital: f64 = match self.initial_capital_str.parse() {
            Ok(v) if v > 0.0 && f64::is_finite(v) => v,
            _ => {
                self.validation_error =
                    Some("Initial capital must be a positive number".to_string());
                return None;
            }
        };

        let commission: f64 = match self.commission_str.parse() {
            Ok(v) if v >= 0.0 && f64::is_finite(v) => v,
            _ => {
                self.validation_error =
                    Some("Commission must be a non-negative number".to_string());
                return None;
            }
        };

        let Some(strategy_id) = self.selected_strategy_id.clone() else {
            self.validation_error = Some("No strategy selected".to_string());
            return None;
        };

        if self.selected_connection_idx.is_none() {
            self.validation_error = Some("No data source selected".to_string());
            return None;
        }

        // Resolve ticker
        let Some(ref ticker_symbol) = self.selected_ticker else {
            self.validation_error = Some("No ticker selected".to_string());
            return None;
        };
        let ticker = data::FuturesTicker::new(ticker_symbol, data::FuturesVenue::CMEGlobex);

        let slippage = match self.slippage_mode {
            SlippageMode::None => SlippageModel::None,
            SlippageMode::FixedTick => {
                let ticks = self.slippage_ticks_str.parse::<i64>().unwrap_or(0);
                SlippageModel::FixedTick(ticks)
            }
            SlippageMode::Percentage => SlippageModel::Percentage(0.0001),
        };

        let pos_val: f64 = self.position_size_value_str.parse().unwrap_or(1.0);
        let position_size_mode = match self.position_size_mode {
            PositionSizeModeUI::Fixed => PositionSizeMode::Fixed(pos_val),
            PositionSizeModeUI::RiskPercent => PositionSizeMode::RiskPercent(pos_val / 100.0),
            PositionSizeModeUI::RiskDollars => PositionSizeMode::RiskDollars(pos_val),
        };

        let max_concurrent: usize = self.max_concurrent_str.parse().unwrap_or(1).max(1);

        let max_drawdown_pct = if self.max_drawdown_enabled {
            self.max_drawdown_pct_str
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && *v > 0.0)
                .map(|v| v / 100.0)
        } else {
            None
        };

        let rth_open: u32 = self.rth_open_str.parse().unwrap_or(930);
        let rth_close: u32 = self.rth_close_str.parse().unwrap_or(1600);

        let date_range = data::DateRange {
            start: self.calendar.start_date,
            end: self.calendar.end_date,
        };

        // Collect strategy params from snapshot
        let strategy_params: HashMap<String, study::ParameterValue> = self
            .strategy_snapshots
            .iter()
            .find(|(id, _)| id == &strategy_id)
            .map(|(_, s)| {
                s.config()
                    .values
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();

        self.validation_error = None;

        let config = BacktestConfig {
            ticker,
            date_range,
            timeframe: self.selected_timeframe,
            initial_capital_usd: initial_capital,
            risk: RiskConfig {
                position_size_mode,
                max_concurrent_positions: max_concurrent,
                max_drawdown_pct,
                ..RiskConfig::default()
            },
            slippage,
            commission_per_side_usd: commission,
            timezone_offset_hours: -5,
            rth_open_hhmm: rth_open,
            rth_close_hhmm: rth_close,
            strategy_id,
            strategy_params,
            additional_instruments: Vec::new(),
            additional_timeframes: Vec::new(),
            warm_up_periods: 0,
            use_depth_data: false,
            margin: Default::default(),
            simulated_latency_ms: 0,
        };

        Some(Action::RunRequested(Box::new(config)))
    }
}
