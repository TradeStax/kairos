//! Data Management Modal - Production Quality
//!
//! Professional data management UI with:
//! - Visual calendar date range selector
//! - Ticker dropdown (no text input)
//! - Real Databento USD cost API integration
//! - Clean layout with proper sections
//! - NO emojis

use crate::{split_column, style, widget::scrollable_content};
use chrono::Datelike; // For year(), month(), day(), weekday()
use data::{DateRange, FuturesTicker};
use exchange::{DatabentoSchema, FuturesVenue};
use iced::{
    Alignment, Color, Element, Length,
    widget::{button, column, container, pick_list, row, space, text},
};

/// Futures products for ticker dropdown
pub const FUTURES_PRODUCTS: &[(&str, &str)] = &[
    ("ES.c.0", "E-mini S&P 500"),
    ("NQ.c.0", "E-mini Nasdaq-100"),
    ("YM.c.0", "E-mini Dow"),
    ("RTY.c.0", "E-mini Russell 2000"),
    ("CL.c.0", "Crude Oil"),
    ("GC.c.0", "Gold"),
    ("SI.c.0", "Silver"),
    ("ZN.c.0", "10-Year T-Note"),
    ("ZB.c.0", "30-Year T-Bond"),
    ("ZF.c.0", "5-Year T-Note"),
    ("NG.c.0", "Natural Gas"),
    ("HG.c.0", "Copper"),
];

/// Schemas with display names
pub const SCHEMAS: &[(DatabentoSchema, &str, u8)] = &[
    (DatabentoSchema::Trades, "Trades", 2),
    (DatabentoSchema::Mbp10, "MBP-10 (10 Levels)", 3),
    (DatabentoSchema::Mbp1, "MBP-1 (Top of Book)", 2),
    (DatabentoSchema::Ohlcv1M, "OHLCV-1M", 1),
    (DatabentoSchema::Tbbo, "TBBO (Top BBO)", 2),
    (DatabentoSchema::Mbo, "MBO (VERY EXPENSIVE)", 10),
];

/// Data management panel state
#[derive(Debug, Clone, PartialEq)]
pub struct DataManagementPanel {
    selected_ticker_idx: usize,
    selected_schema_idx: usize,
    calendar: DateRangeCalendar,

    cache_status: Option<CacheStatus>,
    cached_dates: Option<std::collections::HashSet<chrono::NaiveDate>>, // For calendar coloring
    actual_cost_usd: Option<f64>,
    download_progress: DownloadProgress,
}

/// Calendar for visual date range selection
#[derive(Debug, Clone, PartialEq)]
struct DateRangeCalendar {
    viewing_month: chrono::NaiveDate, // First day of month being viewed
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
    selection_mode: SelectionMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SelectionMode {
    SelectingStart,
    SelectingEnd,
}

/// Cache coverage status
#[derive(Debug, Clone, PartialEq)]
pub struct CacheStatus {
    pub total_days: usize,
    pub cached_days: usize,
    pub uncached_days: usize,
    pub gaps_description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadProgress {
    Idle,
    CheckingCost,
    Downloading { current_day: usize, total_days: usize },
    Complete { days_downloaded: usize },
    Error(String),
}

#[derive(Debug, Clone)]
pub enum DataManagementMessage {
    TickerSelected(usize),
    SchemaSelected(usize),

    // Calendar
    PrevMonth,
    NextMonth,
    DayClicked(chrono::NaiveDate),

    // Actions
    Download,
    CancelDownload,
}

pub enum Action {
    EstimateRequested {
        ticker: FuturesTicker,
        schema: DatabentoSchema,
        date_range: DateRange,
    },
    DownloadRequested {
        ticker: FuturesTicker,
        schema: DatabentoSchema,
        date_range: DateRange,
    },
}

/// Custom style for calendar day buttons
/// Text color = cache status, Outline = selection status
fn calendar_day_style(
    base_text_color: Color,
    is_selected: bool,
) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style {
    move |theme, status| {
        let palette = theme.extended_palette();

        iced::widget::button::Style {
            // Text color based ONLY on cache status
            text_color: match status {
                iced::widget::button::Status::Hovered => {
                    // Subtle dim on hover (85% opacity)
                    Color::from_rgba(
                        base_text_color.r,
                        base_text_color.g,
                        base_text_color.b,
                        base_text_color.a * 0.85,
                    )
                }
                _ => base_text_color, // Cache status color always
            },

            // NO background ever
            background: None,

            // Outline ONLY for selected dates
            border: if is_selected {
                iced::Border {
                    width: 1.5,
                    color: palette.primary.strong.color, // Theme primary color
                    radius: 3.0.into(),
                }
            } else {
                iced::Border::default() // No outline
            },

            shadow: iced::Shadow::default(),
            snap: true,
        }
    }
}

impl DataManagementPanel {
    pub fn new() -> Self {
        let today = chrono::Utc::now().date_naive();
        let start = today - chrono::Duration::days(6); // Last 7 days default

        Self {
            selected_ticker_idx: 0, // ES.c.0 default
            selected_schema_idx: 0, // Trades default
            calendar: DateRangeCalendar {
                viewing_month: chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap(),
                start_date: start,
                end_date: today,
                selection_mode: SelectionMode::SelectingStart,
            },
            cache_status: None,
            cached_dates: None,
            actual_cost_usd: None,
            download_progress: DownloadProgress::Idle,
        }
    }

    pub fn with_ticker(mut self, ticker: FuturesTicker) -> Self {
        // Find ticker in FUTURES_PRODUCTS
        let ticker_str = ticker.to_string();
        if let Some(idx) = FUTURES_PRODUCTS.iter().position(|(sym, _)| *sym == ticker_str) {
            self.selected_ticker_idx = idx;
        }
        self
    }

    pub fn update(&mut self, message: DataManagementMessage) -> Option<Action> {
        match message {
            DataManagementMessage::TickerSelected(idx) => {
                self.selected_ticker_idx = idx;
                self.cache_status = None;
                self.actual_cost_usd = None;
                return self.trigger_estimation();
            }
            DataManagementMessage::SchemaSelected(idx) => {
                self.selected_schema_idx = idx;
                self.cache_status = None;
                self.actual_cost_usd = None;
                return self.trigger_estimation();
            }
            DataManagementMessage::PrevMonth => {
                let prev = self.calendar.viewing_month - chrono::Months::new(1);
                self.calendar.viewing_month = chrono::NaiveDate::from_ymd_opt(prev.year(), prev.month(), 1).unwrap();
                // Auto-estimate to get cache status for new month
                return self.trigger_estimation();
            }
            DataManagementMessage::NextMonth => {
                let next = self.calendar.viewing_month + chrono::Months::new(1);
                self.calendar.viewing_month = chrono::NaiveDate::from_ymd_opt(next.year(), next.month(), 1).unwrap();
                // Auto-estimate to get cache status for new month
                return self.trigger_estimation();
            }
            DataManagementMessage::DayClicked(date) => {
                match self.calendar.selection_mode {
                    SelectionMode::SelectingStart => {
                        self.calendar.start_date = date;
                        self.calendar.end_date = date; // Reset end to start
                        self.calendar.selection_mode = SelectionMode::SelectingEnd;
                    }
                    SelectionMode::SelectingEnd => {
                        if date >= self.calendar.start_date {
                            self.calendar.end_date = date;
                            self.calendar.selection_mode = SelectionMode::SelectingStart; // Ready for next selection
                            self.cache_status = None;
                            self.actual_cost_usd = None;
                            return self.trigger_estimation();
                        } else {
                            // If clicked before start, make it the new start
                            self.calendar.start_date = date;
                        }
                    }
                }
                self.cache_status = None;
                self.actual_cost_usd = None;
            }
            DataManagementMessage::Download => {
                if self.actual_cost_usd.is_some() {
                    let num_days = (self.calendar.end_date - self.calendar.start_date).num_days() + 1;
                    self.download_progress = DownloadProgress::Downloading {
                        current_day: 0,
                        total_days: num_days as usize,
                    };
                    let ticker = FuturesTicker::new(FUTURES_PRODUCTS[self.selected_ticker_idx].0, FuturesVenue::CMEGlobex);
                    let schema = SCHEMAS[self.selected_schema_idx].0;
                    let date_range = DateRange::new(self.calendar.start_date, self.calendar.end_date);
                    return Some(Action::DownloadRequested { ticker, schema, date_range });
                }
            }
            DataManagementMessage::CancelDownload => {
                self.download_progress = DownloadProgress::Idle;
            }
        }
        None
    }

    fn trigger_estimation(&mut self) -> Option<Action> {
        self.download_progress = DownloadProgress::CheckingCost;
        let ticker = FuturesTicker::new(FUTURES_PRODUCTS[self.selected_ticker_idx].0, FuturesVenue::CMEGlobex);
        let schema = SCHEMAS[self.selected_schema_idx].0;
        let date_range = DateRange::new(self.calendar.start_date, self.calendar.end_date);
        Some(Action::EstimateRequested { ticker, schema, date_range })
    }

    pub fn set_cache_status(&mut self, status: CacheStatus, cached_dates: Vec<chrono::NaiveDate>) {
        self.cache_status = Some(status);
        self.cached_dates = Some(cached_dates.into_iter().collect());
    }

    pub fn set_actual_cost(&mut self, cost_usd: f64) {
        self.actual_cost_usd = Some(cost_usd);
        self.download_progress = DownloadProgress::Idle;
    }

    pub fn set_download_progress(&mut self, progress: DownloadProgress) {
        self.download_progress = progress;
    }

    pub fn selected_ticker_idx(&self) -> usize {
        self.selected_ticker_idx
    }

    pub fn selected_schema_idx(&self) -> usize {
        self.selected_schema_idx
    }

    pub fn current_date_range(&self) -> DateRange {
        DateRange::new(self.calendar.start_date, self.calendar.end_date)
    }

    pub fn view<'a>(&'a self) -> Element<'a, DataManagementMessage> {
        // Ticker dropdown section
        let ticker_section = {
            let (symbol, name) = FUTURES_PRODUCTS[self.selected_ticker_idx];
            let ticker_options: Vec<String> = FUTURES_PRODUCTS
                .iter()
                .map(|(sym, name)| format!("{} - {}", sym, name))
                .collect();

            column![
                text("Ticker").size(13),
                pick_list(
                    ticker_options,
                    Some(format!("{} - {}", symbol, name)),
                    |selected| {
                        FUTURES_PRODUCTS
                            .iter()
                            .position(|(sym, n)| format!("{} - {}", sym, n) == selected)
                            .map(DataManagementMessage::TickerSelected)
                            .unwrap_or_else(|| DataManagementMessage::TickerSelected(0))
                    }
                )
                .width(Length::Fill),
            ]
            .spacing(4)
        };

        // Schema dropdown section
        let schema_section = {
            let (_schema, name, cost_rating) = SCHEMAS[self.selected_schema_idx];
            let schema_options: Vec<String> = SCHEMAS
                .iter()
                .map(|(_, name, rating)| format!("{} (Cost: {}/10)", name, rating))
                .collect();

            column![
                text("Schema").size(13),
                pick_list(
                    schema_options,
                    Some(format!("{} (Cost: {}/10)", name, cost_rating)),
                    |selected| {
                        SCHEMAS
                            .iter()
                            .position(|(_, n, r)| format!("{} (Cost: {}/10)", n, r) == selected)
                            .map(DataManagementMessage::SchemaSelected)
                            .unwrap_or_else(|| DataManagementMessage::SchemaSelected(0))
                    }
                )
                .width(Length::Fill),
            ]
            .spacing(4)
        };

        // Date range calendar section
        let calendar_section = column![
            text("Date Range").size(13),
            row![
                text("From:"),
                text(self.calendar.start_date.format("%b %d, %Y").to_string()).size(11),
                space::horizontal(),
                text("To:"),
                text(self.calendar.end_date.format("%b %d, %Y").to_string()).size(11),
            ]
            .spacing(4),
            self.calendar_view(),
        ]
        .spacing(6);

        // Action button - shows progress in button text itself
        let (download_button_text, is_downloading) = match &self.download_progress {
            DownloadProgress::Downloading { current_day, total_days } => {
                (format!("Downloading {}/{}", current_day, total_days), true)
            }
            DownloadProgress::CheckingCost => {
                ("Checking cost...".to_string(), false)
            }
            _ => {
                // Idle or Complete - show cost
                if let Some(cost) = self.actual_cost_usd {
                    if cost < 0.01 {
                        ("Download (Free)".to_string(), false)
                    } else {
                        (format!("Download (${:.4})", cost), false)
                    }
                } else {
                    ("Download (calculating...)".to_string(), false)
                }
            }
        };

        // Can download if: have cost, not currently downloading
        let can_download = self.actual_cost_usd.is_some()
            && !is_downloading
            && !matches!(self.download_progress, DownloadProgress::CheckingCost);

        let action_buttons = button(text(download_button_text))
            .on_press_maybe(if can_download { Some(DataManagementMessage::Download) } else { None })
            .style(|t, s| style::button::confirm(t, s, false));

        // Build modal with clean sections (no status text section)
        let content = split_column![
            ticker_section,
            schema_section,
            calendar_section,
            action_buttons
            ; spacing = 10, align_x = Alignment::Start
        ];

        container(scrollable_content(content))
            .width(Length::Fixed(360.0))
            .padding(20)
            .style(style::chart_modal)
            .into()
    }

    fn calendar_view(&self) -> Element<DataManagementMessage> {
        let month = self.calendar.viewing_month;

        // Month/year header with navigation
        let header = row![
            button(text("<").size(14))
                .on_press(DataManagementMessage::PrevMonth)
                .style(|t, s| style::button::transparent(t, s, false))
                .width(Length::Fixed(28.0)),
            text(month.format("%B %Y").to_string())
                .size(13)
                .width(Length::Fill)
                .align_x(Alignment::Center),
            button(text(">").size(14))
                .on_press(DataManagementMessage::NextMonth)
                .style(|t, s| style::button::transparent(t, s, false))
                .width(Length::Fixed(28.0)),
        ]
        .align_y(Alignment::Center);

        // Day of week headers (Mon-Fri only)
        let dow_headers = row![
            text("Mon").size(10).width(Length::FillPortion(1)).align_x(Alignment::Center),
            text("Tue").size(10).width(Length::FillPortion(1)).align_x(Alignment::Center),
            text("Wed").size(10).width(Length::FillPortion(1)).align_x(Alignment::Center),
            text("Thu").size(10).width(Length::FillPortion(1)).align_x(Alignment::Center),
            text("Fri").size(10).width(Length::FillPortion(1)).align_x(Alignment::Center),
        ]
        .spacing(2);

        // Calendar grid
        let grid = self.build_calendar_grid(month);

        column![
            header,
            dow_headers,
            grid,
        ]
        .spacing(4)
        .into()
    }

    fn build_calendar_grid(&self, month: chrono::NaiveDate) -> Element<DataManagementMessage> {
        use chrono::Weekday;

        let today = chrono::Utc::now().date_naive();

        // Find first Monday of viewing period (may be before month starts)
        let first_day = chrono::NaiveDate::from_ymd_opt(month.year(), month.month(), 1).unwrap();
        let days_until_monday = match first_day.weekday() {
            Weekday::Mon => 0,
            Weekday::Tue => 1,
            Weekday::Wed => 2,
            Weekday::Thu => 3,
            Weekday::Fri => 4,
            Weekday::Sat => 5,
            Weekday::Sun => 6,
        };
        let calendar_start = first_day - chrono::Duration::days(days_until_monday);

        let start = self.calendar.start_date;
        let end = self.calendar.end_date;

        let mut grid = column![].spacing(4); // Vertical gaps between weeks

        // Build 6 weeks × 5 weekdays (30 buttons total)
        for week in 0..6 {
            let mut week_row = row![].spacing(4); // Gaps between buttons

            for day in 0..5 {
                let date = calendar_start + chrono::Duration::days(week * 7 + day);

                // Don't show future dates - render NOTHING (not even empty button)
                if date > today {
                    week_row = week_row.push(space::horizontal().width(Length::FillPortion(1)));
                    continue;
                }

                let is_current_month = date.month() == month.month() && date.year() == month.year();
                let is_in_range = date >= start && date <= end;
                let is_start = date == start;
                let is_end = date == end;
                let is_cached = self.cached_dates.as_ref().map(|set| set.contains(&date)).unwrap_or(false);

                // Text color based ONLY on cache status
                let base_text_color = if !is_current_month {
                    // Other month - very dim gray
                    Color::from_rgba(0.5, 0.5, 0.5, 0.3)
                } else if is_cached {
                    // Cached - full brightness
                    Color::from_rgba(1.0, 1.0, 1.0, 1.0)
                } else {
                    // Uncached - dimmed (needs download)
                    Color::from_rgba(1.0, 1.0, 1.0, 0.5)
                };

                // Outline ONLY for selected dates
                let is_selected = is_in_range;

                // Day number text (size 10 for all - consistent)
                let day_text = text(format!("{}", date.day()))
                    .size(10)
                    .align_x(Alignment::Center);

                let day_button = button(day_text)
                    .width(Length::FillPortion(1))
                    .height(Length::Fixed(26.0))
                    .style(calendar_day_style(base_text_color, is_selected))
                    .on_press(DataManagementMessage::DayClicked(date));

                week_row = week_row.push(day_button);
            }

            grid = grid.push(week_row);
        }

        grid.into()
    }
}

impl Default for DataManagementPanel {
    fn default() -> Self {
        Self::new()
    }
}
