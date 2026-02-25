//! Historical Data Download Modal
//!
//! Stacked modal for downloading historical datasets from Databento.
//! Uses shared calendar component and download views.

use super::views;
use super::{CacheStatus, DownloadConfig, DownloadProgress};
use crate::components::overlay::modal_header::ModalHeaderBuilder;
use crate::modals::pane::calendar::{CalendarMessage, DateRangeCalendar};
use crate::style::{self, tokens};
use data::{DateRange, FuturesTicker};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, space, text},
};

/// Historical download modal state
#[derive(Debug, Clone, PartialEq)]
pub struct HistoricalDownloadModal {
    selected_ticker_idx: usize,
    selected_schema_idx: usize,
    calendar: DateRangeCalendar,

    cache_status: Option<CacheStatus>,
    actual_cost_usd: Option<f64>,
    has_valid_selection: bool,

    download_progress: DownloadProgress,
    show_confirm: bool,
}

#[derive(Debug, Clone)]
pub enum HistoricalDownloadMessage {
    TickerSelected(usize),
    SchemaSelected(usize),
    Calendar(CalendarMessage),
    ShowConfirm,
    ConfirmDownload,
    CancelDownload,
    Close,
}

pub enum Action {
    EstimateRequested {
        ticker: FuturesTicker,
        schema: exchange::DownloadSchema,
        date_range: DateRange,
    },
    DownloadRequested {
        ticker: FuturesTicker,
        schema: exchange::DownloadSchema,
        date_range: DateRange,
    },
    Closed,
}

impl HistoricalDownloadModal {
    pub fn new() -> Self {
        Self {
            selected_ticker_idx: 0,
            selected_schema_idx: 0,
            calendar: DateRangeCalendar::new(),
            cache_status: None,
            actual_cost_usd: None,
            has_valid_selection: false,
            download_progress: DownloadProgress::Idle,
            show_confirm: false,
        }
    }

    pub fn update(&mut self, message: HistoricalDownloadMessage) -> Option<Action> {
        match message {
            HistoricalDownloadMessage::TickerSelected(idx) => {
                self.selected_ticker_idx = idx;
                self.cache_status = None;
                self.actual_cost_usd = None;
                return self.trigger_viewing_month_check();
            }
            HistoricalDownloadMessage::SchemaSelected(idx) => {
                self.selected_schema_idx = idx;
                self.cache_status = None;
                self.actual_cost_usd = None;
                return self.trigger_viewing_month_check();
            }
            HistoricalDownloadMessage::Calendar(cal_msg) => {
                let is_month_nav = matches!(
                    cal_msg,
                    CalendarMessage::PrevMonth | CalendarMessage::NextMonth
                );
                let selection_complete = self.calendar.update(cal_msg);
                self.cache_status = None;
                self.actual_cost_usd = None;

                if is_month_nav {
                    return self.trigger_viewing_month_check();
                } else if selection_complete {
                    return self.trigger_estimation();
                }
            }
            HistoricalDownloadMessage::ShowConfirm => {
                if self.actual_cost_usd.is_some() {
                    self.show_confirm = true;
                }
            }
            HistoricalDownloadMessage::ConfirmDownload => {
                self.show_confirm = false;

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
                let date_range = DateRange::new(self.calendar.start_date, self.calendar.end_date).ok()?;
                return Some(Action::DownloadRequested {
                    ticker,
                    schema,
                    date_range,
                });
            }
            HistoricalDownloadMessage::CancelDownload => {
                self.show_confirm = false;
                self.download_progress = DownloadProgress::Idle;
            }
            HistoricalDownloadMessage::Close => {
                return Some(Action::Closed);
            }
        }
        None
    }

    fn trigger_estimation(&mut self) -> Option<Action> {
        self.download_progress = DownloadProgress::CheckingCost;
        let ticker = DownloadConfig::ticker_from_idx(self.selected_ticker_idx);
        let schema = DownloadConfig::schema_from_idx(self.selected_schema_idx);
        let date_range = DateRange::new(self.calendar.start_date, self.calendar.end_date).ok()?;
        Some(Action::EstimateRequested {
            ticker,
            schema,
            date_range,
        })
    }

    fn trigger_viewing_month_check(&mut self) -> Option<Action> {
        self.download_progress = DownloadProgress::CheckingCost;
        let (first, last) = self.calendar.viewing_month_range();
        let ticker = DownloadConfig::ticker_from_idx(self.selected_ticker_idx);
        let schema = DownloadConfig::schema_from_idx(self.selected_schema_idx);
        Some(Action::EstimateRequested {
            ticker,
            schema,
            date_range: DateRange::new(first, last).ok()?,
        })
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

    pub fn auto_name(&self) -> String {
        let (sym, _) = super::FUTURES_PRODUCTS[self.selected_ticker_idx];
        let short_sym = sym.split('.').next().unwrap_or(sym);
        let (_, schema_name, _) = super::SCHEMAS[self.selected_schema_idx];
        let start_fmt = self.calendar.start_date.format("%b%d").to_string();
        let end_fmt = self.calendar.end_date.format("%b%d").to_string();
        format!("{} {} {}-{}", short_sym, schema_name, start_fmt, end_fmt)
    }

    pub fn selected_ticker_idx(&self) -> usize {
        self.selected_ticker_idx
    }

    pub fn selected_schema_idx(&self) -> usize {
        self.selected_schema_idx
    }

    pub fn view(&self) -> Element<'_, HistoricalDownloadMessage> {
        let header = ModalHeaderBuilder::new("Download Historical Data")
            .on_close(HistoricalDownloadMessage::Close);

        let source_label = text("Source: Databento").size(tokens::text::BODY);

        let ticker_section = views::ticker_dropdown(
            self.selected_ticker_idx,
            HistoricalDownloadMessage::TickerSelected,
        );

        let schema_section = views::schema_dropdown(
            self.selected_schema_idx,
            HistoricalDownloadMessage::SchemaSelected,
        );

        let calendar_section = column![
            row![
                text("From:").size(tokens::text::SMALL),
                text(self.calendar.start_date.format("%b %d, %Y").to_string())
                    .size(tokens::text::SMALL),
                space::horizontal(),
                text("To:").size(tokens::text::SMALL),
                text(self.calendar.end_date.format("%b %d, %Y").to_string())
                    .size(tokens::text::SMALL),
            ]
            .spacing(tokens::spacing::XS),
            self.calendar.view(HistoricalDownloadMessage::Calendar),
        ]
        .spacing(tokens::spacing::XS);

        let cache_line: Element<'_, HistoricalDownloadMessage> =
            views::cache_status_display(&self.download_progress, self.cache_status.as_ref());

        let progress_section: Option<Element<'_, HistoricalDownloadMessage>> =
            views::download_progress_section(&self.download_progress);

        // Action buttons
        let is_downloading = matches!(self.download_progress, DownloadProgress::Downloading { .. });
        let can_download = self.has_valid_selection
            && !is_downloading
            && !matches!(self.download_progress, DownloadProgress::CheckingCost);

        let buttons = row![
            button(
                text("Cancel")
                    .size(tokens::text::LABEL)
                    .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .on_press(HistoricalDownloadMessage::Close)
            .padding([tokens::spacing::MD, tokens::spacing::XL]),
            button(
                text(if is_downloading {
                    "Downloading..."
                } else {
                    "Download"
                })
                .size(tokens::text::LABEL)
                .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .on_press_maybe(if can_download {
                Some(HistoricalDownloadMessage::ShowConfirm)
            } else {
                None
            })
            .padding([tokens::spacing::MD, tokens::spacing::XL])
            .style(style::button::primary),
        ]
        .spacing(tokens::spacing::MD);

        let mut content_items: Vec<Element<'_, HistoricalDownloadMessage>> = vec![
            source_label.into(),
            ticker_section,
            schema_section,
            calendar_section.into(),
            cache_line,
        ];

        if let Some(progress) = progress_section {
            content_items.push(progress);
        }

        content_items.push(buttons.into());

        let content = content_items.into_iter().fold(
            column![]
                .spacing(tokens::spacing::LG)
                .align_x(Alignment::Start),
            |col, item| col.push(item),
        );

        let body = container(content).padding(iced::Padding {
            top: tokens::spacing::MD,
            right: tokens::spacing::XXL,
            bottom: tokens::spacing::XXL,
            left: tokens::spacing::XXL,
        });

        let base_modal = container(
            column![header, body].width(Length::Fill),
        )
        .width(Length::Fixed(tokens::layout::MODAL_WIDTH_LG))
        .style(style::dashboard_modal);

        if self.show_confirm {
            views::download_confirm_overlay(
                base_modal.into(),
                self.selected_ticker_idx,
                self.selected_schema_idx,
                self.calendar.start_date,
                self.calendar.end_date,
                self.actual_cost_usd.unwrap_or(0.0),
                self.cache_status.as_ref(),
                HistoricalDownloadMessage::CancelDownload,
                HistoricalDownloadMessage::ConfirmDownload,
            )
        } else {
            base_modal.into()
        }
    }
}
