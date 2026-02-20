//! Data Management Modal
//!
//! Pane-level data management UI with:
//! - Visual calendar date range selector
//! - Ticker dropdown
//! - Real Databento USD cost API integration

use super::views;
use super::{CacheStatus, DownloadConfig, DownloadProgress};
use crate::components::layout::scrollable_content::scrollable_content;
use crate::modals::pane::calendar::{CalendarMessage, DateRangeCalendar};
use crate::style::{self, tokens};
use data::{DateRange, FuturesTicker};
use exchange::DownloadSchema;
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, space, text},
};

/// Data management panel state
#[derive(Debug, Clone, PartialEq)]
pub struct DataManagementPanel {
    selected_ticker_idx: usize,
    selected_schema_idx: usize,
    calendar: DateRangeCalendar,

    cache_status: Option<CacheStatus>,
    actual_cost_usd: Option<f64>,
    download_progress: DownloadProgress,
    show_confirm_modal: bool,
    has_valid_selection: bool,
}

#[derive(Debug, Clone)]
pub enum DataManagementMessage {
    TickerSelected(usize),
    SchemaSelected(usize),
    Calendar(CalendarMessage),
    ShowDownloadConfirm,
    ConfirmDownload,
    CancelDownload,
}

pub enum Action {
    EstimateRequested {
        ticker: FuturesTicker,
        schema: DownloadSchema,
        date_range: DateRange,
    },
    DownloadRequested {
        ticker: FuturesTicker,
        schema: DownloadSchema,
        date_range: DateRange,
    },
}

impl DataManagementPanel {
    pub fn new() -> Self {
        Self {
            selected_ticker_idx: 0,
            selected_schema_idx: 0,
            calendar: DateRangeCalendar::new(),
            cache_status: None,
            actual_cost_usd: None,
            download_progress: DownloadProgress::Idle,
            show_confirm_modal: false,
            has_valid_selection: false,
        }
    }

    pub fn with_ticker(mut self, ticker: FuturesTicker) -> Self {
        let ticker_str = ticker.to_string();
        if let Some(idx) = super::FUTURES_PRODUCTS
            .iter()
            .position(|(sym, _)| *sym == ticker_str)
        {
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
                if self.actual_cost_usd.is_some() {
                    self.show_confirm_modal = true;
                }
            }
            DataManagementMessage::ConfirmDownload => {
                self.show_confirm_modal = false;
                let num_days = (self.calendar.end_date - self.calendar.start_date)
                    .num_days()
                    .max(0)
                    + 1;
                self.download_progress = DownloadProgress::Downloading {
                    current_day: 0,
                    total_days: num_days as usize,
                };
                let ticker = DownloadConfig::ticker_from_idx(self.selected_ticker_idx);
                let schema = DownloadConfig::schema_from_idx(self.selected_schema_idx);
                let date_range = DateRange::new(self.calendar.start_date, self.calendar.end_date);
                return Some(Action::DownloadRequested {
                    ticker,
                    schema,
                    date_range,
                });
            }
            DataManagementMessage::CancelDownload => {
                self.show_confirm_modal = false;
                self.download_progress = DownloadProgress::Idle;
            }
        }
        None
    }

    fn trigger_estimation(&mut self, date_range: Option<DateRange>) -> Option<Action> {
        self.download_progress = DownloadProgress::CheckingCost;
        let ticker = DownloadConfig::ticker_from_idx(self.selected_ticker_idx);
        let schema = DownloadConfig::schema_from_idx(self.selected_schema_idx);
        let range = date_range
            .unwrap_or_else(|| DateRange::new(self.calendar.start_date, self.calendar.end_date));
        Some(Action::EstimateRequested {
            ticker,
            schema,
            date_range: range,
        })
    }

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
        let (start, end) = if self.calendar.end_date >= self.calendar.start_date {
            (self.calendar.start_date, self.calendar.end_date)
        } else {
            (self.calendar.end_date, self.calendar.start_date)
        };
        DateRange::new(start, end)
    }

    fn viewing_month_range(&self) -> DateRange {
        let (first, last) = self.calendar.viewing_month_range();
        DateRange::new(first, last)
    }

    pub fn request_initial_estimation(&mut self) -> Option<Action> {
        if !matches!(
            self.download_progress,
            DownloadProgress::CheckingCost | DownloadProgress::Downloading { .. }
        ) {
            self.trigger_viewing_month_cache_check()
        } else {
            None
        }
    }

    pub fn view(&self) -> Element<'_, DataManagementMessage> {
        let ticker_section = views::ticker_dropdown(
            self.selected_ticker_idx,
            DataManagementMessage::TickerSelected,
        );

        let schema_section = views::schema_dropdown(
            self.selected_schema_idx,
            DataManagementMessage::SchemaSelected,
        );

        let calendar_section = column![
            text("Date Range").size(tokens::text::LABEL),
            row![
                text("From:"),
                text(self.calendar.start_date.format("%b %d, %Y").to_string())
                    .size(tokens::text::SMALL),
                space::horizontal(),
                text("To:"),
                text(self.calendar.end_date.format("%b %d, %Y").to_string())
                    .size(tokens::text::SMALL),
            ]
            .spacing(tokens::spacing::XS),
            self.calendar.view(DataManagementMessage::Calendar),
        ]
        .spacing(tokens::spacing::SM);

        let cache_summary =
            views::cache_status_display(&self.download_progress, self.cache_status.as_ref());

        let progress_section: Option<Element<'_, DataManagementMessage>> =
            views::download_progress_section(&self.download_progress);

        // Action button
        let (download_button_text, is_downloading) = match &self.download_progress {
            DownloadProgress::Downloading { .. } => ("Downloading...", true),
            DownloadProgress::CheckingCost => ("Checking...", false),
            _ => ("Download", false),
        };

        let can_download = self.has_valid_selection
            && self.actual_cost_usd.is_some()
            && !is_downloading
            && !matches!(self.download_progress, DownloadProgress::CheckingCost);

        let action_button = button(
            text(download_button_text)
                .size(tokens::text::LABEL)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([tokens::spacing::MD, tokens::spacing::XL])
        .on_press_maybe(if can_download {
            Some(DataManagementMessage::ShowDownloadConfirm)
        } else {
            None
        })
        .style(style::button::primary);

        let mut content_items: Vec<Element<'_, DataManagementMessage>> = vec![
            ticker_section,
            schema_section,
            calendar_section.into(),
            cache_summary,
        ];

        if let Some(progress) = progress_section {
            content_items.push(progress);
        }

        content_items.push(action_button.into());

        let base_content = content_items.into_iter().fold(
            column![]
                .spacing(tokens::spacing::LG)
                .align_x(Alignment::Start),
            |col, item| col.push(item),
        );

        let base_modal = container(scrollable_content(base_content))
            .width(Length::Fixed(tokens::layout::MODAL_WIDTH_MD))
            .padding(tokens::spacing::XXL)
            .style(style::chart_modal);

        if self.show_confirm_modal {
            views::download_confirm_overlay(
                base_modal.into(),
                self.selected_ticker_idx,
                self.selected_schema_idx,
                self.calendar.start_date,
                self.calendar.end_date,
                self.actual_cost_usd.unwrap_or(0.0),
                self.cache_status.as_ref(),
                DataManagementMessage::CancelDownload,
                DataManagementMessage::ConfirmDownload,
            )
        } else {
            base_modal.into()
        }
    }
}

impl Default for DataManagementPanel {
    fn default() -> Self {
        Self::new()
    }
}
