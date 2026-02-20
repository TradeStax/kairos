//! Shared Calendar Component
//!
//! Reusable date range calendar used by DataManagementPanel and
//! HistoricalDownloadModal.

use crate::components::primitives::label::{label_text, tiny};
use crate::style;
use crate::style::tokens;
use chrono::{Datelike, NaiveDate, Weekday};
use iced::{
    Alignment, Color, Element, Length,
    widget::{button, column, container, row, space, text},
};
use std::collections::HashSet;

/// Calendar for visual date range selection
#[derive(Debug, Clone, PartialEq)]
pub struct DateRangeCalendar {
    /// First day of the month being viewed
    pub viewing_month: NaiveDate,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub selection_mode: SelectionMode,
    /// Which dates are cached (for coloring)
    pub cached_dates: Option<HashSet<NaiveDate>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectionMode {
    SelectingStart,
    SelectingEnd,
}

/// Messages emitted by the calendar
#[derive(Debug, Clone)]
pub enum CalendarMessage {
    PrevMonth,
    NextMonth,
    DayClicked(NaiveDate),
}

impl DateRangeCalendar {
    pub fn new() -> Self {
        let yesterday = chrono::Utc::now().date_naive() - chrono::Duration::days(1);
        let start = yesterday - chrono::Duration::days(6);

        Self {
            viewing_month: NaiveDate::from_ymd_opt(yesterday.year(), yesterday.month(), 1).unwrap(),
            start_date: start,
            end_date: yesterday,
            selection_mode: SelectionMode::SelectingStart,
            cached_dates: None,
        }
    }

    /// Handle a calendar message. Returns true if the selection is complete
    /// (end date picked), false otherwise.
    pub fn update(&mut self, message: CalendarMessage) -> bool {
        match message {
            CalendarMessage::PrevMonth => {
                let prev = self.viewing_month - chrono::Months::new(1);
                self.viewing_month = NaiveDate::from_ymd_opt(prev.year(), prev.month(), 1).unwrap();
                false
            }
            CalendarMessage::NextMonth => {
                let next = self.viewing_month + chrono::Months::new(1);
                self.viewing_month = NaiveDate::from_ymd_opt(next.year(), next.month(), 1).unwrap();
                false
            }
            CalendarMessage::DayClicked(date) => {
                match self.selection_mode {
                    SelectionMode::SelectingStart => {
                        self.start_date = date;
                        self.end_date = date;
                        self.selection_mode = SelectionMode::SelectingEnd;
                        false
                    }
                    SelectionMode::SelectingEnd => {
                        if date >= self.start_date {
                            self.end_date = date;
                        } else {
                            self.end_date = self.start_date;
                            self.start_date = date;
                        }
                        self.selection_mode = SelectionMode::SelectingStart;
                        true // selection complete
                    }
                }
            }
        }
    }

    /// Get the viewing month date range (first to last day)
    pub fn viewing_month_range(&self) -> (NaiveDate, NaiveDate) {
        let month = self.viewing_month;
        let first_day = NaiveDate::from_ymd_opt(month.year(), month.month(), 1).unwrap();
        let next_month = if month.month() == 12 {
            NaiveDate::from_ymd_opt(month.year() + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(month.year(), month.month() + 1, 1).unwrap()
        };
        let last_day = next_month - chrono::Duration::days(1);
        (first_day, last_day)
    }

    pub fn view<M: Clone + 'static>(
        &self,
        map_msg: impl Fn(CalendarMessage) -> M + 'static + Copy,
    ) -> Element<'_, M> {
        let month = self.viewing_month;

        let header = row![
            button(text("<").size(14))
                .on_press(map_msg(CalendarMessage::PrevMonth))
                .style(|t, s| style::button::transparent(t, s, false))
                .width(Length::Fixed(28.0)),
            label_text(month.format("%B %Y").to_string())
                .width(Length::Fill)
                .align_x(Alignment::Center),
            button(text(">").size(14))
                .on_press(map_msg(CalendarMessage::NextMonth))
                .style(|t, s| style::button::transparent(t, s, false))
                .width(Length::Fixed(28.0)),
        ]
        .align_y(Alignment::Center);

        let dow_headers = row![
            tiny("Mon")
                .width(Length::FillPortion(1))
                .align_x(Alignment::Center),
            tiny("Tue")
                .width(Length::FillPortion(1))
                .align_x(Alignment::Center),
            tiny("Wed")
                .width(Length::FillPortion(1))
                .align_x(Alignment::Center),
            tiny("Thu")
                .width(Length::FillPortion(1))
                .align_x(Alignment::Center),
            tiny("Fri")
                .width(Length::FillPortion(1))
                .align_x(Alignment::Center),
        ]
        .spacing(tokens::spacing::XXS);

        let grid = self.build_grid(map_msg);

        container(column![header, dow_headers, grid].spacing(tokens::spacing::XS))
            .padding(tokens::spacing::LG)
            .style(style::modal_container)
            .into()
    }

    fn build_grid<M: Clone + 'static>(
        &self,
        map_msg: impl Fn(CalendarMessage) -> M + 'static + Copy,
    ) -> Element<'_, M> {
        let today = chrono::Utc::now().date_naive();
        let month = self.viewing_month;

        let first_day = NaiveDate::from_ymd_opt(month.year(), month.month(), 1).unwrap();
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

        let start = self.start_date;
        let end = self.end_date;
        let yesterday = today - chrono::Duration::days(1);

        let mut grid = column![].spacing(tokens::spacing::XS);

        for week in 0..6 {
            let mut week_row = row![].spacing(tokens::spacing::XS);

            for day in 0..5 {
                let date = calendar_start + chrono::Duration::days(week * 7 + day);

                if date > yesterday {
                    week_row = week_row.push(space::horizontal().width(Length::FillPortion(1)));
                    continue;
                }

                let is_current_month = date.month() == month.month() && date.year() == month.year();
                let is_in_range = date >= start && date <= end;
                let is_cached = self
                    .cached_dates
                    .as_ref()
                    .map(|set| set.contains(&date))
                    .unwrap_or(false);

                let base_text_color = if !is_current_month {
                    Color::from_rgba(0.5, 0.5, 0.5, 0.3)
                } else if is_cached {
                    Color::from_rgba(1.0, 1.0, 1.0, 1.0)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.5)
                };

                let day_text = tiny(format!("{}", date.day())).align_x(Alignment::Center);

                let day_button = button(day_text)
                    .width(Length::FillPortion(1))
                    .height(Length::Fixed(26.0))
                    .style(calendar_day_style(base_text_color, is_in_range, is_cached))
                    .on_press_maybe(if is_cached {
                        None
                    } else {
                        Some(map_msg(CalendarMessage::DayClicked(date)))
                    });

                week_row = week_row.push(day_button);
            }

            grid = grid.push(week_row);
        }

        grid.into()
    }
}

/// Custom style for calendar day buttons
fn calendar_day_style(
    base_text_color: Color,
    is_selected: bool,
    is_cached: bool,
) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style {
    move |theme, status| {
        let palette = theme.extended_palette();

        iced::widget::button::Style {
            text_color: match status {
                iced::widget::button::Status::Hovered => Color::from_rgba(
                    base_text_color.r,
                    base_text_color.g,
                    base_text_color.b,
                    base_text_color.a * 0.85,
                ),
                _ => base_text_color,
            },
            background: if is_cached {
                Some(Color::from_rgba(0.5, 0.5, 0.5, 0.2).into())
            } else {
                None
            },
            border: if is_selected {
                iced::Border {
                    width: tokens::border::MEDIUM,
                    color: palette.primary.strong.color,
                    radius: 3.0.into(),
                }
            } else {
                iced::Border {
                    width: tokens::border::NONE,
                    color: Color::TRANSPARENT,
                    radius: 3.0.into(),
                }
            },
            shadow: iced::Shadow::default(),
            snap: true,
        }
    }
}
