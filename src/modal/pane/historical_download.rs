//! Historical Data Download Modal
//!
//! Stacked modal for downloading historical datasets from Databento.
//! Uses the shared calendar component for date range selection.

use super::calendar::{CalendarMessage, DateRangeCalendar};
use super::{FUTURES_PRODUCTS, SCHEMAS};
use crate::style;
use data::{DateRange, FuturesTicker};
use exchange::FuturesVenue;
use iced::{
    Alignment, Color, Element, Length,
    widget::{
        button, center, column, container, mouse_area, opaque,
        pick_list, progress_bar, row, space, stack, text, text_input,
    },
};

/// Historical download modal state
#[derive(Debug, Clone, PartialEq)]
pub struct HistoricalDownloadModal {
    selected_ticker_idx: usize,
    selected_schema_idx: usize,
    calendar: DateRangeCalendar,

    api_key_input: String,
    api_key_stored: bool,

    cache_status: Option<CacheStatus>,
    actual_cost_usd: Option<f64>,
    has_valid_selection: bool,

    download_progress: DownloadProgress,
    show_confirm: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CacheStatus {
    pub total_days: usize,
    pub cached_days: usize,
    pub uncached_days: usize,
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
pub enum HistoricalDownloadMessage {
    TickerSelected(usize),
    SchemaSelected(usize),
    Calendar(CalendarMessage),
    SetApiKey(String),
    ShowConfirm,
    ConfirmDownload,
    CancelDownload,
    Close,
}

pub enum Action {
    EstimateRequested {
        ticker: FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: DateRange,
    },
    DownloadRequested {
        ticker: FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: DateRange,
    },
    DatasetCreated(data::DataFeed),
    ApiKeySaved {
        provider: data::ApiProvider,
        key: String,
    },
    Closed,
}

impl HistoricalDownloadModal {
    pub fn new() -> Self {
        let api_key_stored = data::SecretsManager::new()
            .has_api_key(data::ApiProvider::Databento);

        Self {
            selected_ticker_idx: 0,
            selected_schema_idx: 0,
            calendar: DateRangeCalendar::new(),
            api_key_input: String::new(),
            api_key_stored,
            cache_status: None,
            actual_cost_usd: None,
            has_valid_selection: false,
            download_progress: DownloadProgress::Idle,
            show_confirm: false,
        }
    }

    pub fn update(
        &mut self,
        message: HistoricalDownloadMessage,
    ) -> Option<Action> {
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
                    CalendarMessage::PrevMonth
                        | CalendarMessage::NextMonth
                );
                let selection_complete =
                    self.calendar.update(cal_msg);
                self.cache_status = None;
                self.actual_cost_usd = None;

                if is_month_nav {
                    return self.trigger_viewing_month_check();
                } else if selection_complete {
                    return self.trigger_estimation();
                }
            }
            HistoricalDownloadMessage::SetApiKey(key) => {
                self.api_key_input = key;
            }
            HistoricalDownloadMessage::ShowConfirm => {
                if self.actual_cost_usd.is_some() {
                    self.show_confirm = true;
                }
            }
            HistoricalDownloadMessage::ConfirmDownload => {
                self.show_confirm = false;

                // Save API key if entered
                if !self.api_key_input.is_empty() {
                    let key = self.api_key_input.clone();
                    self.api_key_stored = true;
                    self.api_key_input.clear();
                    // We'll save the key as a separate action,
                    // then also trigger the download
                    let num_days = (self.calendar.end_date
                        - self.calendar.start_date)
                        .num_days()
                        .max(0)
                        + 1;
                    self.download_progress =
                        DownloadProgress::Downloading {
                            current_day: 0,
                            total_days: num_days as usize,
                        };
                    return Some(Action::ApiKeySaved {
                        provider: data::ApiProvider::Databento,
                        key,
                    });
                }

                let num_days = (self.calendar.end_date
                    - self.calendar.start_date)
                    .num_days()
                    .max(0)
                    + 1;
                self.download_progress =
                    DownloadProgress::Downloading {
                        current_day: 0,
                        total_days: num_days as usize,
                    };
                let ticker = FuturesTicker::new(
                    FUTURES_PRODUCTS[self.selected_ticker_idx].0,
                    FuturesVenue::CMEGlobex,
                );
                let schema = SCHEMAS[self.selected_schema_idx].0;
                let date_range = DateRange::new(
                    self.calendar.start_date,
                    self.calendar.end_date,
                );
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
        let ticker = FuturesTicker::new(
            FUTURES_PRODUCTS[self.selected_ticker_idx].0,
            FuturesVenue::CMEGlobex,
        );
        let schema = SCHEMAS[self.selected_schema_idx].0;
        let date_range = DateRange::new(
            self.calendar.start_date,
            self.calendar.end_date,
        );
        Some(Action::EstimateRequested {
            ticker,
            schema,
            date_range,
        })
    }

    fn trigger_viewing_month_check(&mut self) -> Option<Action> {
        self.download_progress = DownloadProgress::CheckingCost;
        let (first, last) = self.calendar.viewing_month_range();
        let ticker = FuturesTicker::new(
            FUTURES_PRODUCTS[self.selected_ticker_idx].0,
            FuturesVenue::CMEGlobex,
        );
        let schema = SCHEMAS[self.selected_schema_idx].0;
        Some(Action::EstimateRequested {
            ticker,
            schema,
            date_range: DateRange::new(first, last),
        })
    }

    pub fn set_cache_status(
        &mut self,
        status: CacheStatus,
        cached_dates: Vec<chrono::NaiveDate>,
    ) {
        self.cache_status = Some(status);
        self.calendar.cached_dates =
            Some(cached_dates.into_iter().collect());
    }

    pub fn set_actual_cost(&mut self, cost_usd: f64) {
        self.actual_cost_usd = Some(cost_usd);
        self.download_progress = DownloadProgress::Idle;
        self.has_valid_selection = true;
    }

    pub fn set_download_progress(&mut self, progress: DownloadProgress) {
        self.download_progress = progress;
    }

    /// Generate the auto-name for a dataset
    pub fn auto_name(&self) -> String {
        let (sym, _) = FUTURES_PRODUCTS[self.selected_ticker_idx];
        let short_sym = sym.split('.').next().unwrap_or(sym);
        let (_, schema_name, _) = SCHEMAS[self.selected_schema_idx];
        let start_fmt =
            self.calendar.start_date.format("%b%d").to_string();
        let end_fmt =
            self.calendar.end_date.format("%b%d").to_string();
        format!("{} {} {}-{}", short_sym, schema_name, start_fmt, end_fmt)
    }

    pub fn selected_ticker_idx(&self) -> usize {
        self.selected_ticker_idx
    }

    pub fn selected_schema_idx(&self) -> usize {
        self.selected_schema_idx
    }

    pub fn current_date_range(&self) -> DateRange {
        let (start, end) = if self.calendar.end_date
            >= self.calendar.start_date
        {
            (self.calendar.start_date, self.calendar.end_date)
        } else {
            (self.calendar.end_date, self.calendar.start_date)
        };
        DateRange::new(start, end)
    }

    // ================================================================
    // View
    // ================================================================

    pub fn view(&self) -> Element<'_, HistoricalDownloadMessage> {
        let title = row![
            text("Download Historical Data").size(16),
            space::horizontal().width(Length::Fill),
            button(
                text("\u{00D7}")
                    .size(14)
                    .align_x(Alignment::Center),
            )
            .width(28)
            .height(28)
            .on_press(HistoricalDownloadMessage::Close),
        ]
        .align_y(Alignment::Center);

        let source_label =
            text("Source: Databento").size(12);

        // Ticker dropdown
        let ticker_section = {
            let (symbol, name) =
                FUTURES_PRODUCTS[self.selected_ticker_idx];
            let ticker_options: Vec<String> = FUTURES_PRODUCTS
                .iter()
                .map(|(sym, name)| format!("{} - {}", sym, name))
                .collect();

            column![
                text("Ticker").size(12),
                pick_list(
                    ticker_options,
                    Some(format!("{} - {}", symbol, name)),
                    |selected| {
                        FUTURES_PRODUCTS
                            .iter()
                            .position(|(sym, n)| {
                                format!("{} - {}", sym, n) == selected
                            })
                            .map(
                                HistoricalDownloadMessage::TickerSelected,
                            )
                            .unwrap_or(
                                HistoricalDownloadMessage::TickerSelected(
                                    0,
                                ),
                            )
                    }
                )
                .width(Length::Fill),
            ]
            .spacing(4)
        };

        // Schema dropdown
        let schema_section = {
            let (_schema, name, cost_rating) =
                SCHEMAS[self.selected_schema_idx];
            let schema_options: Vec<String> = SCHEMAS
                .iter()
                .map(|(_, name, rating)| {
                    format!("{} (Cost: {}/10)", name, rating)
                })
                .collect();

            column![
                text("Schema").size(12),
                pick_list(
                    schema_options,
                    Some(format!(
                        "{} (Cost: {}/10)",
                        name, cost_rating
                    )),
                    |selected| {
                        SCHEMAS
                            .iter()
                            .position(|(_, n, r)| {
                                format!("{} (Cost: {}/10)", n, r)
                                    == selected
                            })
                            .map(
                                HistoricalDownloadMessage::SchemaSelected,
                            )
                            .unwrap_or(
                                HistoricalDownloadMessage::SchemaSelected(
                                    0,
                                ),
                            )
                    }
                )
                .width(Length::Fill),
            ]
            .spacing(4)
        };

        // Calendar
        let calendar_section = column![
            row![
                text("From:").size(11),
                text(
                    self.calendar
                        .start_date
                        .format("%b %d, %Y")
                        .to_string()
                )
                .size(11),
                space::horizontal(),
                text("To:").size(11),
                text(
                    self.calendar
                        .end_date
                        .format("%b %d, %Y")
                        .to_string()
                )
                .size(11),
            ]
            .spacing(4),
            self.calendar.view(HistoricalDownloadMessage::Calendar),
        ]
        .spacing(4);

        // Cache status line
        let cache_line: Element<'_, HistoricalDownloadMessage> =
            if matches!(
                self.download_progress,
                DownloadProgress::CheckingCost
            ) {
                text("Checking...").size(11).into()
            } else if let Some(ref status) = self.cache_status {
                let num_selected = (self.calendar.end_date
                    - self.calendar.start_date)
                    .num_days()
                    .max(0)
                    + 1;
                text(format!(
                    "{} days selected ({} cached)",
                    num_selected, status.cached_days
                ))
                .size(11)
                .into()
            } else {
                text("Select a date range").size(11).into()
            };

        // API key field
        let api_key_section: Element<'_, HistoricalDownloadMessage> =
            if self.api_key_stored {
                row![
                    text("API Key:").size(12),
                    text("saved")
                        .size(12)
                        .style(|theme: &iced::Theme| {
                            iced::widget::text::Style {
                                color: Some(
                                    theme
                                        .extended_palette()
                                        .success
                                        .base
                                        .color,
                                ),
                            }
                        }),
                ]
                .spacing(6)
                .into()
            } else {
                column![
                    text("API Key").size(12),
                    text_input(
                        "Enter Databento API key",
                        &self.api_key_input,
                    )
                    .on_input(HistoricalDownloadMessage::SetApiKey)
                    .secure(true)
                    .size(13),
                ]
                .spacing(4)
                .into()
            };

        // Progress section
        let progress_section: Option<
            Element<'_, HistoricalDownloadMessage>,
        > = match &self.download_progress {
            DownloadProgress::Downloading {
                current_day,
                total_days,
            } => {
                let pct = if *total_days > 0 {
                    (*current_day as f32 / *total_days as f32)
                        * 100.0
                } else {
                    0.0
                };
                Some(
                    column![
                        row![
                            text("Downloading...").size(12),
                            space::horizontal().width(Length::Fill),
                            text(format!(
                                "{}/{} days ({}%)",
                                current_day,
                                total_days,
                                pct as u32
                            ))
                            .size(11),
                        ]
                        .align_y(Alignment::Center),
                        progress_bar(0.0..=100.0, pct)
                            .girth(6.0)
                            .style(style::progress_bar),
                    ]
                    .spacing(4)
                    .into(),
                )
            }
            DownloadProgress::Complete { days_downloaded } => Some(
                text(format!(
                    "Download complete - {} days",
                    days_downloaded
                ))
                .size(12)
                .style(|theme: &iced::Theme| {
                    iced::widget::text::Style {
                        color: Some(
                            theme
                                .extended_palette()
                                .success
                                .base
                                .color,
                        ),
                    }
                })
                .into(),
            ),
            DownloadProgress::Error(err) => Some(
                text(format!("Error: {}", err))
                    .size(12)
                    .style(|theme: &iced::Theme| {
                        iced::widget::text::Style {
                            color: Some(
                                theme
                                    .extended_palette()
                                    .danger
                                    .base
                                    .color,
                            ),
                        }
                    })
                    .into(),
            ),
            _ => None,
        };

        // Action buttons
        let is_downloading = matches!(
            self.download_progress,
            DownloadProgress::Downloading { .. }
        );
        let can_download = self.has_valid_selection
            && (self.api_key_stored
                || !self.api_key_input.is_empty())
            && !is_downloading
            && !matches!(
                self.download_progress,
                DownloadProgress::CheckingCost
            );

        let buttons = row![
            button(
                text("Cancel")
                    .size(13)
                    .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .on_press(HistoricalDownloadMessage::Close)
            .padding([8, 16]),
            button(
                text(if is_downloading {
                    "Downloading..."
                } else {
                    "Download"
                })
                .size(13)
                .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .on_press_maybe(if can_download {
                Some(HistoricalDownloadMessage::ShowConfirm)
            } else {
                None
            })
            .padding([8, 16])
            .style(style::button::primary),
        ]
        .spacing(8);

        // Build content
        let mut content_items: Vec<
            Element<'_, HistoricalDownloadMessage>,
        > = vec![
            title.into(),
            source_label.into(),
            ticker_section.into(),
            schema_section.into(),
            calendar_section.into(),
            cache_line,
            api_key_section,
        ];

        if let Some(progress) = progress_section {
            content_items.push(progress);
        }

        content_items.push(buttons.into());

        let content = content_items.into_iter().fold(
            column![].spacing(10).align_x(Alignment::Start),
            |col, item| col.push(item),
        );

        let base_modal = container(content)
            .width(Length::Fixed(420.0))
            .padding(20)
            .style(style::dashboard_modal);

        if self.show_confirm {
            self.confirmation_overlay(base_modal.into())
        } else {
            base_modal.into()
        }
    }

    fn confirmation_overlay<'a>(
        &'a self,
        base: Element<'a, HistoricalDownloadMessage>,
    ) -> Element<'a, HistoricalDownloadMessage> {
        let cost = self.actual_cost_usd.unwrap_or(0.0);
        let (symbol, name) =
            FUTURES_PRODUCTS[self.selected_ticker_idx];
        let (_, schema_name, _) =
            SCHEMAS[self.selected_schema_idx];
        let total_days = (self.calendar.end_date
            - self.calendar.start_date)
            .num_days()
            .max(0)
            + 1;
        let cached_days = self
            .cache_status
            .as_ref()
            .map(|s| s.cached_days.min(total_days as usize))
            .unwrap_or(0);
        let uncached_days =
            (total_days as usize).saturating_sub(cached_days);

        let cost_text = if cached_days == total_days as usize {
            text("Cost: Free (all data cached)").size(15).style(
                |theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(
                        theme
                            .extended_palette()
                            .success
                            .base
                            .color,
                    ),
                },
            )
        } else if cost < 0.01 {
            text("Cost: $0.00 (may be incorrect)").size(15).style(
                |theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(
                        theme
                            .extended_palette()
                            .danger
                            .base
                            .color,
                    ),
                },
            )
        } else {
            text(format!("Cost: ${:.4} USD", cost)).size(15)
        };

        let confirm_content = container(
            column![
                text("Confirm Download").size(18),
                space::vertical().height(Length::Fixed(12.0)),
                text(format!("{} - {}", symbol, name)).size(14),
                text(format!("Schema: {}", schema_name)).size(13),
                text(format!(
                    "Date Range: {} to {}",
                    self.calendar
                        .start_date
                        .format("%b %d, %Y"),
                    self.calendar.end_date.format("%b %d, %Y")
                ))
                .size(13),
                space::vertical().height(Length::Fixed(8.0)),
                text(format!(
                    "{} days total ({} cached, {} to download)",
                    total_days, cached_days, uncached_days
                ))
                .size(12),
                space::vertical().height(Length::Fixed(12.0)),
                cost_text,
                space::vertical().height(Length::Fixed(16.0)),
                row![
                    button(
                        text("Cancel")
                            .size(13)
                            .align_x(Alignment::Center)
                    )
                    .on_press(
                        HistoricalDownloadMessage::CancelDownload
                    )
                    .width(Length::Fill)
                    .padding([10, 16])
                    .style(style::button::secondary),
                    button(
                        text("Confirm")
                            .size(13)
                            .align_x(Alignment::Center)
                    )
                    .on_press(
                        HistoricalDownloadMessage::ConfirmDownload
                    )
                    .width(Length::Fill)
                    .padding([10, 16])
                    .style(style::button::primary),
                ]
                .spacing(10)
            ]
            .spacing(6)
            .padding(20)
            .align_x(Alignment::Center),
        )
        .width(Length::Fixed(340.0))
        .style(style::confirm_modal);

        stack![
            base,
            opaque(
                container(
                    mouse_area(center(opaque(confirm_content)))
                        .on_press(
                            HistoricalDownloadMessage::CancelDownload
                        )
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
