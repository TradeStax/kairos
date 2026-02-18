//! Data Management Modal - Production Quality
//!
//! Professional data management UI with:
//! - Visual calendar date range selector
//! - Ticker dropdown (no text input)
//! - Real Databento USD cost API integration
//! - Clean layout with proper sections
//! - NO emojis

use crate::{style, widget::scrollable_content};
use data::{DateRange, FuturesTicker};
use exchange::{DatabentoSchema, FuturesVenue};
use iced::{
    Alignment, Color, Element, Length,
    widget::{button, center, column, container, mouse_area, opaque, pick_list, progress_bar, row, space, stack, text},
};
use super::calendar::{DateRangeCalendar, CalendarMessage};
use super::{FUTURES_PRODUCTS, SCHEMAS};

/// Data management panel state
#[derive(Debug, Clone, PartialEq)]
pub struct DataManagementPanel {
    selected_ticker_idx: usize,
    selected_schema_idx: usize,
    calendar: DateRangeCalendar,

    cache_status: Option<CacheStatus>,
    actual_cost_usd: Option<f64>,
    download_progress: DownloadProgress,
    show_confirm_modal: bool, // Show download confirmation modal
    has_valid_selection: bool, // True after user has selected a date range
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

    // Calendar (delegated to shared component)
    Calendar(CalendarMessage),

    // Actions
    ShowDownloadConfirm, // Show confirmation modal
    ConfirmDownload, // User confirmed download
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

impl DataManagementPanel {
    pub fn new() -> Self {
        Self {
            selected_ticker_idx: 0, // ES.c.0 default
            selected_schema_idx: 0, // Trades default
            calendar: DateRangeCalendar::new(),
            cache_status: None,
            actual_cost_usd: None,
            download_progress: DownloadProgress::Idle,
            show_confirm_modal: false,
            has_valid_selection: false,
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
                return self.trigger_viewing_month_cache_check();
            }
            DataManagementMessage::SchemaSelected(idx) => {
                self.selected_schema_idx = idx;
                self.cache_status = None;
                self.actual_cost_usd = None;
                return self.trigger_viewing_month_cache_check();
            }
            DataManagementMessage::Calendar(cal_msg) => {
                let is_month_nav = matches!(
                    cal_msg,
                    CalendarMessage::PrevMonth | CalendarMessage::NextMonth
                );
                let selection_complete = self.calendar.update(cal_msg);

                if is_month_nav {
                    return self.trigger_viewing_month_cache_check();
                } else if selection_complete {
                    self.cache_status = None;
                    self.actual_cost_usd = None;
                    return self.trigger_estimation(None);
                }
                return None;
            }
            DataManagementMessage::ShowDownloadConfirm => {
                // Show confirmation modal (only if cost is available)
                if self.actual_cost_usd.is_some() {
                    self.show_confirm_modal = true;
                }
            }
            DataManagementMessage::ConfirmDownload => {
                // User confirmed - proceed with download
                self.show_confirm_modal = false;
                let num_days = (self.calendar.end_date - self.calendar.start_date).num_days().max(0) + 1;
                self.download_progress = DownloadProgress::Downloading {
                    current_day: 0,
                    total_days: num_days as usize,
                };
                let ticker = FuturesTicker::new(FUTURES_PRODUCTS[self.selected_ticker_idx].0, FuturesVenue::CMEGlobex);
                let schema = SCHEMAS[self.selected_schema_idx].0;
                let date_range = DateRange::new(self.calendar.start_date, self.calendar.end_date);
                return Some(Action::DownloadRequested { ticker, schema, date_range });
            }
            DataManagementMessage::CancelDownload => {
                self.show_confirm_modal = false;
                self.download_progress = DownloadProgress::Idle;
            }
        }
        None
    }

    /// Trigger cache estimation for a specific date range
    /// If no range provided, uses the currently selected range
    fn trigger_estimation(&mut self, date_range: Option<DateRange>) -> Option<Action> {
        self.download_progress = DownloadProgress::CheckingCost;
        let ticker = FuturesTicker::new(FUTURES_PRODUCTS[self.selected_ticker_idx].0, FuturesVenue::CMEGlobex);
        let schema = SCHEMAS[self.selected_schema_idx].0;
        let range = date_range.unwrap_or_else(|| DateRange::new(self.calendar.start_date, self.calendar.end_date));
        Some(Action::EstimateRequested { ticker, schema, date_range: range })
    }

    /// Trigger cache check for the entire viewing month (not just selected range)
    /// This is used when navigating months or opening the modal to show accurate cache status
    fn trigger_viewing_month_cache_check(&mut self) -> Option<Action> {
        let viewing_range = self.viewing_month_range();
        self.trigger_estimation(Some(viewing_range))
    }

    pub fn set_cache_status(&mut self, status: CacheStatus, cached_dates: Vec<chrono::NaiveDate>) {
        self.cache_status = Some(status);
        self.calendar.cached_dates = Some(cached_dates.into_iter().collect());
    }

    pub fn set_actual_cost(&mut self, cost_usd: f64) {
        self.actual_cost_usd = Some(cost_usd);
        self.download_progress = DownloadProgress::Idle;
        self.has_valid_selection = true;
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
        // Ensure dates are always valid (start <= end)
        let (start, end) = if self.calendar.end_date >= self.calendar.start_date {
            (self.calendar.start_date, self.calendar.end_date)
        } else {
            (self.calendar.end_date, self.calendar.start_date) // Swap if backwards
        };
        DateRange::new(start, end)
    }

    fn viewing_month_range(&self) -> DateRange {
        let (first, last) = self.calendar.viewing_month_range();
        DateRange::new(first, last)
    }

    /// Request initial cache status estimation (called when modal opens)
    /// Always triggers estimation to refresh cached dates for the entire viewing month
    pub fn request_initial_estimation(&mut self) -> Option<Action> {
        // Always trigger viewing month check when modal opens to show accurate cache status
        if !matches!(self.download_progress, DownloadProgress::CheckingCost | DownloadProgress::Downloading { .. }) {
            self.trigger_viewing_month_cache_check()
        } else {
            None
        }
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
                text(self.calendar.start_date.format("%b %d, %Y").to_string())
                    .size(11),
                space::horizontal(),
                text("To:"),
                text(self.calendar.end_date.format("%b %d, %Y").to_string())
                    .size(11),
            ]
            .spacing(4),
            self.calendar.view(DataManagementMessage::Calendar),
        ]
        .spacing(6);

        // Cache status summary - CLEARLY shows what's already downloaded (NO COST)
        let cache_summary = if matches!(self.download_progress, DownloadProgress::CheckingCost) {
            // Show "Checking cost..." when estimating
            column![
                text("Checking cost...").size(11)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().primary.base.color),
                    }),
            ]
        } else if let Some(ref status) = self.cache_status {
            let total_days = status.total_days;
            let cached_days = status.cached_days;
            let uncached_days = status.uncached_days;

            let summary_text = if cached_days == total_days {
                text(format!("✓ All {} days already downloaded", total_days))
                    .size(12)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().success.base.color),
                    })
            } else if cached_days > 0 {
                text(format!("○ {}/{} days cached ({} to download)",
                    cached_days, total_days, uncached_days))
                    .size(12)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().primary.base.color),
                    })
            } else {
                text(format!("⬇ Need to download all {} days", total_days))
                    .size(12)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().secondary.base.color),
                    })
            };

            // NO COST INFO HERE - cost only shown in confirmation modal
            column![
                summary_text,
            ]
            .spacing(2)
        } else {
            column![
                text("Select date range to see cache status").size(11),
            ]
        };

        // Visual progress section
        let progress_section: Option<Element<'_, DataManagementMessage>> = match &self.download_progress {
            DownloadProgress::Downloading { current_day, total_days } => {
                let progress_pct = if *total_days > 0 {
                    (*current_day as f32 / *total_days as f32) * 100.0
                } else {
                    0.0
                };

                Some(
                    container(
                        column![
                            row![
                                text("Downloading...").size(12),
                                space::horizontal(),
                                text(format!("{}/{} days ({}%)", current_day, total_days, progress_pct as u32)).size(11),
                            ].align_y(Alignment::Center),
                            progress_bar(0.0..=100.0, progress_pct)
                                .girth(6.0)
                                .style(style::progress_bar),
                        ]
                        .spacing(6)
                    )
                    .padding(12)
                    .style(style::modal_container)
                    .into()
                )
            }
            DownloadProgress::Complete { days_downloaded } => {
                Some(
                    container(
                        text(format!("Download complete - {} days", days_downloaded))
                            .size(12)
                            .style(|theme: &iced::Theme| iced::widget::text::Style {
                                color: Some(theme.extended_palette().success.base.color),
                            })
                    )
                    .padding(10)
                    .style(style::modal_container)
                    .into()
                )
            }
            DownloadProgress::Error(err) => {
                Some(
                    container(
                        text(format!("Error: {}", err))
                            .size(12)
                            .style(|theme: &iced::Theme| iced::widget::text::Style {
                                color: Some(theme.extended_palette().danger.base.color),
                            })
                    )
                    .padding(10)
                    .style(style::modal_container)
                    .into()
                )
            }
            _ => None,
        };

        // Action button - simplified text (no progress in button)
        let (download_button_text, is_downloading) = match &self.download_progress {
            DownloadProgress::Downloading { .. } => ("Downloading...", true),
            DownloadProgress::CheckingCost => ("Checking...", false),
            _ => ("Download", false),
        };

        // Can download if: have valid selection with cost, not currently downloading
        let can_download = self.has_valid_selection
            && self.actual_cost_usd.is_some()
            && !is_downloading
            && !matches!(self.download_progress, DownloadProgress::CheckingCost);

        let action_buttons = button(
            text(download_button_text).size(13).align_x(Alignment::Center)
        )
        .width(Length::Fill)
        .padding([10, 16])
        .on_press_maybe(if can_download { Some(DataManagementMessage::ShowDownloadConfirm) } else { None })
        .style(style::button::primary);

        // Build content with optional progress section
        let mut content_items: Vec<Element<'_, DataManagementMessage>> = vec![
            ticker_section.into(),
            schema_section.into(),
            calendar_section.into(),
            cache_summary.into(),
        ];

        if let Some(progress) = progress_section {
            content_items.push(progress);
        }

        content_items.push(action_buttons.into());

        let base_content = content_items.into_iter().fold(
            column![].spacing(10).align_x(Alignment::Start),
            |col, item| col.push(item)
        );

        let base_modal = container(scrollable_content(base_content))
            .width(Length::Fixed(360.0))
            .padding(20)
            .style(style::chart_modal);

        // If confirmation modal is shown, overlay it on top
        if self.show_confirm_modal {
            self.confirmation_overlay(base_modal.into())
        } else {
            base_modal.into()
        }
    }

    /// Build confirmation modal overlay - proper full-screen overlay
    fn confirmation_overlay<'a>(&'a self, base_content: Element<'a, DataManagementMessage>) -> Element<'a, DataManagementMessage> {
        let cost = self.actual_cost_usd.unwrap_or(0.0);
        let (symbol, name) = FUTURES_PRODUCTS[self.selected_ticker_idx];
        let (_, schema_name, _) = SCHEMAS[self.selected_schema_idx];
        // Ensure end >= start and use saturating subtraction to prevent overflow
        let total_days = (self.calendar.end_date - self.calendar.start_date).num_days().max(0) + 1;
        let cached_days = self
            .cache_status
            .as_ref()
            .map(|s| s.cached_days.min(total_days as usize)) // Cap at total_days
            .unwrap_or(0);
        let uncached_days = (total_days as usize).saturating_sub(cached_days);

        let cost_text = if cached_days == total_days as usize {
            text("Cost: Free (all data cached)")
                .size(15)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().success.base.color),
                })
        } else if cost < 0.01 {
            text("Cost: $0.00 (may be incorrect)")
                .size(15)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                })
        } else {
            text(format!("Cost: ${:.4} USD", cost))
                .size(15)
        };

        let confirmation_content = container(
            column![
                text("Confirm Download").size(18),
                space::vertical().height(Length::Fixed(12.0)),
                text(format!("{} - {}", symbol, name)).size(14),
                text(format!("Schema: {}", schema_name)).size(13),
                text(format!("Date Range: {} to {}",
                    self.calendar.start_date.format("%b %d, %Y"),
                    self.calendar.end_date.format("%b %d, %Y")
                )).size(13),
                space::vertical().height(Length::Fixed(8.0)),
                text(format!("{} days total ({} cached, {} to download)",
                    total_days, cached_days, uncached_days))
                    .size(12),
                space::vertical().height(Length::Fixed(12.0)),
                cost_text,
                space::vertical().height(Length::Fixed(16.0)),
                row![
                    button(text("Cancel").size(13).align_x(Alignment::Center))
                        .on_press(DataManagementMessage::CancelDownload)
                        .width(Length::Fill)
                        .padding([10, 16])
                        .style(style::button::secondary),
                    button(text("Confirm").size(13).align_x(Alignment::Center))
                        .on_press(DataManagementMessage::ConfirmDownload)
                        .width(Length::Fill)
                        .padding([10, 16])
                        .style(style::button::primary),
                ]
                .spacing(10)
            ]
            .spacing(6)
            .padding(20)
            .align_x(Alignment::Center)
        )
        .width(Length::Fixed(340.0))
        .style(style::confirm_modal);

        // Use proper stack-based overlay with semi-transparent background
        stack![
            base_content,
            opaque(
                container(
                    mouse_area(center(opaque(confirmation_content)))
                        .on_press(DataManagementMessage::CancelDownload) // Click outside to cancel
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_theme| {
                    container::Style {
                        background: Some(
                            Color {
                                a: 0.85,
                                ..Color::BLACK
                            }
                            .into(),
                        ),
                        ..container::Style::default()
                    }
                })
            )
        ]
        .into()
    }

}

impl Default for DataManagementPanel {
    fn default() -> Self {
        Self::new()
    }
}
