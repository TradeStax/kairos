//! Level Detail modal — left/right split layout for monitored price levels.
//!
//! Opened via the study overlay detail icon button. Displays all levels
//! from a `LevelAnalyzerStudy` with a selectable list on the left and
//! tabbed detail panel on the right.

mod detail;
mod list;

use iced::{
    Element,
    widget::{column, container, row, rule},
};

use study::orderflow::level_analyzer::types::{
    LevelAnalyzerData, LevelSource, LevelStatus, MonitoredLevel, SessionKey, SessionType,
};

use crate::components::overlay::modal_header::ModalHeaderBuilder;
use crate::components::primitives;
use crate::style;

/// Source filter dropdown options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFilter {
    All,
    Profile,
    Session,
    PriorDay,
    Manual,
    Delta,
    OpeningRange,
}

impl std::fmt::Display for SourceFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All Sources"),
            Self::Profile => write!(f, "Profile"),
            Self::Session => write!(f, "Session"),
            Self::PriorDay => write!(f, "Prior Day"),
            Self::Manual => write!(f, "Manual"),
            Self::Delta => write!(f, "Delta"),
            Self::OpeningRange => write!(f, "Opening Range"),
        }
    }
}

/// Session filter dropdown options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionFilter {
    All,
    CurrentSession,
    RthOnly,
    EthOnly,
}

impl std::fmt::Display for SessionFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All Sessions"),
            Self::CurrentSession => write!(f, "Current"),
            Self::RthOnly => write!(f, "RTH Only"),
            Self::EthOnly => write!(f, "ETH Only"),
        }
    }
}

/// Status filter dropdown options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    All,
    Untested,
    Holding,
    BeingTested,
    Weakening,
    Broken,
}

impl std::fmt::Display for StatusFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All Status"),
            Self::Untested => write!(f, "Untested"),
            Self::Holding => write!(f, "Holding"),
            Self::BeingTested => write!(f, "Testing"),
            Self::Weakening => write!(f, "Weakening"),
            Self::Broken => write!(f, "Broken"),
        }
    }
}

/// Detail panel tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Overview,
    Touches,
    Flow,
}

/// Messages for the level detail modal.
#[derive(Debug, Clone)]
pub enum Message {
    SelectLevel(usize),
    FilterSource(SourceFilter),
    FilterStatus(StatusFilter),
    FilterSession(SessionFilter),
    SwitchTab(DetailTab),
    ManualPriceChanged(String),
    AddManualLevel,
    RemoveSelected,
    Close,
}

/// Actions emitted to the parent pane.
pub enum Action {
    AddLevel(Box<MonitoredLevel>),
    RemoveLevel {
        price_units: i64,
        source: LevelSource,
    },
    CenterOnPrice(f64),
    Close,
}

/// Level detail modal state.
pub struct LevelDetailModal {
    study_index: usize,
    levels: Vec<MonitoredLevel>,
    sessions: Vec<SessionKey>,
    selected_index: Option<usize>,
    source_filter: SourceFilter,
    status_filter: StatusFilter,
    session_filter: SessionFilter,
    active_tab: DetailTab,
    manual_price_input: String,
}

impl std::fmt::Debug for LevelDetailModal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LevelDetailModal")
            .field("study_index", &self.study_index)
            .field("levels_count", &self.levels.len())
            .finish()
    }
}

impl PartialEq for LevelDetailModal {
    fn eq(&self, other: &Self) -> bool {
        self.study_index == other.study_index
    }
}

impl Clone for LevelDetailModal {
    fn clone(&self) -> Self {
        Self {
            study_index: self.study_index,
            levels: self.levels.clone(),
            sessions: self.sessions.clone(),
            selected_index: self.selected_index,
            source_filter: self.source_filter,
            status_filter: self.status_filter,
            session_filter: self.session_filter.clone(),
            active_tab: self.active_tab,
            manual_price_input: self.manual_price_input.clone(),
        }
    }
}

impl LevelDetailModal {
    pub fn new(study_index: usize, data: &LevelAnalyzerData) -> Self {
        Self {
            study_index,
            levels: data.levels.clone(),
            sessions: data.sessions.clone(),
            selected_index: None,
            source_filter: SourceFilter::All,
            status_filter: StatusFilter::All,
            session_filter: SessionFilter::All,
            active_tab: DetailTab::Overview,
            manual_price_input: String::new(),
        }
    }

    pub fn study_index(&self) -> usize {
        self.study_index
    }

    /// Refresh the modal's level data from the study.
    pub fn refresh_levels(&mut self, data: &LevelAnalyzerData) {
        self.levels = data.levels.clone();
        self.sessions = data.sessions.clone();
        // Clamp selected index if it's now out of bounds
        if let Some(idx) = self.selected_index {
            let count = self.filtered_sorted_levels().len();
            if idx >= count {
                self.selected_index = if count > 0 { Some(count - 1) } else { None };
            }
        }
    }

    /// Update modal state and return an optional action.
    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::SelectLevel(idx) => {
                self.selected_index = Some(idx);
                let filtered = self.filtered_sorted_levels();
                filtered
                    .get(idx)
                    .map(|level| Action::CenterOnPrice(level.price))
            }
            Message::FilterSource(f) => {
                self.source_filter = f;
                self.selected_index = None;
                None
            }
            Message::FilterStatus(f) => {
                self.status_filter = f;
                self.selected_index = None;
                None
            }
            Message::FilterSession(f) => {
                self.session_filter = f;
                self.selected_index = None;
                None
            }
            Message::SwitchTab(tab) => {
                self.active_tab = tab;
                None
            }
            Message::ManualPriceChanged(s) => {
                self.manual_price_input = s;
                None
            }
            Message::AddManualLevel => {
                if let Ok(price) = self.manual_price_input.parse::<f64>() {
                    if !price.is_finite() || price <= 0.0 {
                        return None;
                    }
                    let price_units = data::Price::from_f64(price).units();
                    let level = MonitoredLevel::new(
                        0,
                        price_units,
                        price,
                        LevelSource::Manual,
                        0,
                        SessionKey::manual(),
                    );
                    self.manual_price_input.clear();
                    Some(Action::AddLevel(Box::new(level)))
                } else {
                    None
                }
            }
            Message::RemoveSelected => {
                if let Some(idx) = self.selected_index {
                    let filtered = self.filtered_sorted_levels();
                    if let Some(level) = filtered.get(idx) {
                        let action = Action::RemoveLevel {
                            price_units: level.price_units,
                            source: level.source,
                        };
                        self.selected_index = None;
                        return Some(action);
                    }
                }
                None
            }
            Message::Close => Some(Action::Close),
        }
    }

    /// Get the currently selected level (from the filtered list).
    fn selected_level(&self) -> Option<&MonitoredLevel> {
        let idx = self.selected_index?;
        let filtered = self.filtered_sorted_levels();
        filtered.into_iter().nth(idx)
    }

    /// Filtered and sorted levels — sorted by status priority then
    /// strength descending.
    fn filtered_sorted_levels(&self) -> Vec<&MonitoredLevel> {
        let mut levels: Vec<&MonitoredLevel> = self
            .levels
            .iter()
            .filter(|l| matches_source_filter(self.source_filter, l.source))
            .filter(|l| matches_status_filter(self.status_filter, l.status))
            .filter(|l| matches_session_filter(&self.session_filter, l))
            .collect();

        levels.sort_by(|a, b| {
            a.status.order().cmp(&b.status.order()).then_with(|| {
                b.strength
                    .partial_cmp(&a.strength)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        levels
    }

    /// Render the modal view.
    pub fn view(&self) -> Element<'_, Message> {
        let filtered = self.filtered_sorted_levels();
        let level_count = filtered.len();

        let header = ModalHeaderBuilder::new("Level Analyzer")
            .push_control(primitives::small(format!("{level_count} levels")))
            .on_close(Message::Close);

        let left = self.view_left_panel(&filtered);
        let right = self.view_right_panel();

        let body = row![left, rule::vertical(1).style(style::split_ruler), right,].height(420);

        container(column![header, body])
            .width(680)
            .style(style::dashboard_modal)
            .into()
    }
}

fn matches_source_filter(filter: SourceFilter, source: LevelSource) -> bool {
    match filter {
        SourceFilter::All => true,
        SourceFilter::Profile => matches!(
            source,
            LevelSource::Hvn
                | LevelSource::Lvn
                | LevelSource::Poc
                | LevelSource::Vah
                | LevelSource::Val
        ),
        SourceFilter::Session => {
            matches!(source, LevelSource::SessionHigh | LevelSource::SessionLow)
        }
        SourceFilter::PriorDay => matches!(
            source,
            LevelSource::PriorDayHigh | LevelSource::PriorDayLow | LevelSource::PriorDayClose
        ),
        SourceFilter::Manual => source == LevelSource::Manual,
        SourceFilter::Delta => {
            matches!(
                source,
                LevelSource::HighDeltaZone | LevelSource::LowDeltaZone
            )
        }
        SourceFilter::OpeningRange => {
            matches!(
                source,
                LevelSource::OpeningRangeHigh | LevelSource::OpeningRangeLow
            )
        }
    }
}

fn matches_session_filter(filter: &SessionFilter, level: &MonitoredLevel) -> bool {
    match filter {
        SessionFilter::All => true,
        SessionFilter::CurrentSession => {
            level.session_key.is_cross_session() || level.source == LevelSource::Manual
        }
        SessionFilter::RthOnly => {
            level.session_key.session_type == SessionType::Rth
                || level.session_key.is_cross_session()
                || level.source == LevelSource::Manual
        }
        SessionFilter::EthOnly => {
            level.session_key.session_type == SessionType::Eth
                || level.session_key.is_cross_session()
                || level.source == LevelSource::Manual
        }
    }
}

fn matches_status_filter(filter: StatusFilter, status: LevelStatus) -> bool {
    match filter {
        StatusFilter::All => true,
        StatusFilter::Untested => status == LevelStatus::Untested,
        StatusFilter::Holding => status == LevelStatus::Holding,
        StatusFilter::BeingTested => status == LevelStatus::BeingTested,
        StatusFilter::Weakening => status == LevelStatus::Weakening,
        StatusFilter::Broken => status == LevelStatus::Broken,
    }
}

/// Status label string.
fn status_label(status: LevelStatus) -> &'static str {
    match status {
        LevelStatus::Untested => "--",
        LevelStatus::Holding => "Hold",
        LevelStatus::BeingTested => "Test",
        LevelStatus::Weakening => "Weak",
        LevelStatus::Broken => "Brkn",
    }
}

/// Theme-aware color for a level status.
fn status_color(status: LevelStatus) -> impl Fn(&iced::Theme) -> iced::Color {
    use crate::style::palette;
    move |theme: &iced::Theme| match status {
        LevelStatus::Holding => palette::success_color(theme),
        LevelStatus::BeingTested => palette::info_color(theme),
        LevelStatus::Weakening => palette::warning_color(theme),
        LevelStatus::Broken => palette::error_color(theme),
        LevelStatus::Untested => palette::neutral_color(theme),
    }
}

/// Format a volume value compactly (K/M suffixes).
fn fmt_volume(v: f64) -> String {
    if v.abs() >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v.abs() >= 1_000.0 {
        format!("{:.1}K", v / 1_000.0)
    } else {
        format!("{:.0}", v)
    }
}

/// Format a signed delta value compactly.
fn fmt_delta(v: f64) -> String {
    if v.abs() >= 1_000_000.0 {
        format!("{:+.1}M", v / 1_000_000.0)
    } else if v.abs() >= 1_000.0 {
        format!("{:+.1}K", v / 1_000.0)
    } else {
        format!("{:+.0}", v)
    }
}

/// Format a millisecond duration into a human-readable string.
fn fmt_duration_ms(ms: u64) -> String {
    let secs = ms / 1_000;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}
