use super::*;

use crate::components::display::progress_bar::ProgressBarBuilder;
use crate::components::display::status_dot::status_badge_themed;
use crate::components::overlay::modal_header::ModalHeaderBuilder;
use crate::components::primitives::icon_button::toolbar_icon;
use crate::components::primitives::label::small;
use crate::components::primitives::{Icon, icon_text};
use crate::style::{self, palette, tokens};
use chrono::{Datelike, NaiveDate, Weekday};
use iced::widget::{
    button, column, container, mouse_area, opaque, row, scrollable, space, stack, text,
};
use iced::{Alignment, Element, Length, Padding};

impl ReplayManager {
    // ── Sidebar Setup Modal ───────────────────────────────────────────

    /// Sidebar popover: setup form or active-replay controls.
    pub fn view_setup_modal(&self, _timezone: UserTimezone) -> Element<'_, Message> {
        let header = ModalHeaderBuilder::new("Replay")
            .on_close(Message::Close);

        let form = if self.data_loaded {
            self.view_setup_active()
        } else {
            self.view_setup_form()
        };

        let base = container(
            column![
                header,
                container(form).padding(tokens::spacing::LG),
            ],
        )
        .width(Length::Fixed(280.0))
        .style(style::dashboard_modal);

        if let Some(popup) = self.active_popup {
            let (popup_content, offset_y) = match popup {
                Popup::StreamPicker => (self.view_stream_popup(), tokens::component::replay::STREAM_POPUP_Y),
                Popup::DatePicker => (self.view_date_popup(), tokens::component::replay::DATETIME_POPUP_Y),
                Popup::TimePicker => (self.view_time_popup(), tokens::component::replay::DATETIME_POPUP_Y),
            };

            let align_x = match popup {
                Popup::TimePicker => Alignment::End,
                _ => Alignment::Start,
            };

            let positioned = container(opaque(popup_content))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(align_x)
                .padding(Padding {
                    top: offset_y,
                    left: tokens::spacing::LG,
                    right: tokens::spacing::LG,
                    ..Padding::ZERO
                });

            stack![
                container(base).height(Length::Fixed(340.0)),
                mouse_area(positioned).on_press(Message::ClosePopups),
            ]
            .into()
        } else {
            base.into()
        }
    }

    /// Setup form: stream trigger, date/time triggers, start button.
    fn view_setup_form(&self) -> iced::widget::Column<'_, Message> {
        let mut col = column![].spacing(tokens::spacing::MD);

        // ── Stream trigger ────────────────────────────────────
        col = col.push(self.view_picker_trigger(
            "Data Source",
            match &self.selected_stream {
                Some(s) => format!("{} \u{00B7} {}", s.ticker, s.display_name(16)),
                None => String::new(),
            },
            "Select a stream\u{2026}",
            self.active_popup == Some(Popup::StreamPicker),
            true,
            Popup::StreamPicker,
        ));

        // ── Date & Time triggers ──────────────────────────────
        let has_stream = self.selected_stream.is_some();
        let has_date = self.is_date_valid();

        let date_display = if has_date {
            NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d")
                .map(|d| d.format("%m/%d/%Y").to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };

        let (h, m, _) = self.parse_time();
        let time_display = format!("{:02}:{:02}", h, m);

        let date_trigger = self.view_picker_trigger(
            "Date",
            date_display,
            "Select\u{2026}",
            self.active_popup == Some(Popup::DatePicker),
            has_stream,
            Popup::DatePicker,
        );

        let time_trigger = self.view_picker_trigger(
            "Time",
            time_display,
            "Select\u{2026}",
            self.active_popup == Some(Popup::TimePicker),
            has_stream,
            Popup::TimePicker,
        );

        col = col.push(row![date_trigger, time_trigger].spacing(tokens::spacing::MD));

        // ── Load / progress ───────────────────────────────────
        if let Some((progress, ref msg)) = self.loading_progress {
            let bar: Element<'_, Message> = ProgressBarBuilder::new(progress, 1.0)
                .label(msg)
                .show_percentage(true)
                .girth(4.0)
                .into();
            col = col.push(bar);
        } else {
            let can_load = has_stream && (self.start_date.is_empty() || self.is_date_valid());

            let btn_content = text("Start Replay")
                .size(tokens::text::BODY)
                .align_x(Alignment::Center);

            let mut load_btn = button(btn_content)
                .width(Length::Fill)
                .padding([tokens::spacing::SM, tokens::spacing::MD])
                .style(style::button::primary);

            if can_load {
                load_btn = load_btn.on_press(Message::LoadData);
            }

            col = col.push(load_btn);
        }

        if let Some(ref err) = self.error {
            col = col.push(small(err.as_str()).style(palette::error_text));
        }

        col
    }

    // ── Picker Trigger ────────────────────────────────────────────────

    fn view_picker_trigger<'a>(
        &'a self,
        label: &'a str,
        value: String,
        placeholder: &'a str,
        is_open: bool,
        enabled: bool,
        popup: Popup,
    ) -> Element<'a, Message> {
        let label_widget = text(label).size(tokens::text::LABEL);

        let display: Element<'_, Message> = if value.is_empty() {
            text(placeholder)
                .size(tokens::text::BODY)
                .style(palette::neutral_text)
                .into()
        } else {
            text(value).size(tokens::text::BODY).into()
        };

        let arrow = icon_text(
            if is_open {
                Icon::ChevronUp
            } else {
                Icon::ChevronDown
            },
            tokens::text::TINY as u16,
        );

        let content = row![display, space::horizontal().width(Length::Fill), arrow]
            .align_y(Alignment::Center);

        let mut btn = button(content)
            .width(Length::Fill)
            .padding([tokens::spacing::SM, tokens::spacing::MD])
            .style(style::button::secondary);

        if enabled {
            btn = btn.on_press(Message::TogglePopup(popup));
        }

        column![label_widget, btn]
            .spacing(tokens::spacing::XS)
            .width(Length::Fill)
            .into()
    }

    // ── Stream Picker Popup ───────────────────────────────────────────

    fn view_stream_popup(&self) -> Element<'_, Message> {
        let mut items = column![].spacing(tokens::spacing::XXS);

        if self.available_streams.is_empty() {
            items = items.push(
                container(
                    text("No historical connections")
                        .size(tokens::text::TINY)
                        .style(palette::neutral_text),
                )
                .width(Length::Fill)
                .padding(tokens::spacing::MD)
                .align_x(Alignment::Center),
            );
        } else {
            for stream in &self.available_streams {
                let is_selected = self.selected_stream.as_ref() == Some(stream);

                let ticker_label = text(stream.ticker.to_string()).size(tokens::text::BODY);

                let detail = text(format!(
                    "{} \u{00B7} {}\u{2013}{}",
                    stream.display_name(14),
                    stream.date_range.start.format("%m/%d"),
                    stream.date_range.end.format("%m/%d"),
                ))
                .size(tokens::text::TINY)
                .style(palette::neutral_text);

                let item = button(column![ticker_label, detail].spacing(tokens::spacing::XXS))
                    .width(Length::Fill)
                    .padding([tokens::spacing::XS, tokens::spacing::MD])
                    .style(move |theme, status| {
                        style::button::menu_body(theme, status, is_selected)
                    })
                    .on_press(Message::SelectStream(stream.clone()));

                items = items.push(item);
            }
        }

        container(scrollable(items).height(Length::Shrink))
            .width(Length::Fill)
            .max_height(180.0)
            .padding(tokens::spacing::XS)
            .style(style::dropdown_container)
            .into()
    }

    // ── Date Picker Popup (Calendar) ──────────────────────────────────

    fn view_date_popup(&self) -> Element<'_, Message> {
        let Some(month_start) = self.calendar_month else {
            return space::vertical().height(0).into();
        };

        let year = month_start.year();
        let month = month_start.month();

        let date_range = self.selected_stream.as_ref().map(|s| s.date_range);
        let selected_date = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d").ok();

        // Navigation header
        let prev_btn = button(icon_text(Icon::SkipBackward, 10))
            .padding(tokens::spacing::XS)
            .style(|t, s| style::button::transparent(t, s, false))
            .on_press(Message::CalendarPrevMonth);

        let next_btn = button(icon_text(Icon::SkipForward, 10))
            .padding(tokens::spacing::XS)
            .style(|t, s| style::button::transparent(t, s, false))
            .on_press(Message::CalendarNextMonth);

        let month_label = text(month_start.format("%b %Y").to_string()).size(tokens::text::BODY);

        let header = row![
            prev_btn,
            space::horizontal().width(Length::Fill),
            month_label,
            space::horizontal().width(Length::Fill),
            next_btn,
        ]
        .align_y(Alignment::Center);

        // Weekday headers
        let weekday_row = {
            let mut r = row![].spacing(tokens::spacing::XXXS);
            for wd in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
                r = r.push(
                    container(
                        text(wd)
                            .size(tokens::text::TINY)
                            .style(palette::neutral_text)
                            .align_x(Alignment::Center),
                    )
                    .width(tokens::component::replay::CALENDAR_CELL),
                );
            }
            r
        };

        // Day grid
        let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
        let offset = first_day.weekday().num_days_from_monday() as usize;
        let total_days = days_in_month(year, month);

        let mut grid = column![].spacing(tokens::spacing::XXXS);
        let mut week_row = row![].spacing(tokens::spacing::XXXS);

        // Leading blanks
        for _ in 0..offset {
            week_row = week_row.push(container(text("")).width(tokens::component::replay::CALENDAR_CELL));
        }

        for day in 1..=total_days {
            let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
            let in_range = date_range.map_or(false, |r| date >= r.start && date <= r.end);
            let is_weekend = date.weekday() == Weekday::Sat || date.weekday() == Weekday::Sun;
            let is_selected = selected_date == Some(date);

            let day_text = text(format!("{}", day))
                .size(tokens::text::TINY)
                .align_x(Alignment::Center)
                .width(Length::Fill);

            let mut day_btn = button(day_text).width(tokens::component::replay::CALENDAR_CELL).padding([7.0, 0.0]);

            if in_range && !is_weekend {
                day_btn =
                    day_btn
                        .on_press(Message::SelectDate(date))
                        .style(move |theme, status| {
                            if is_selected {
                                style::button::primary(theme, status)
                            } else {
                                style::button::transparent(theme, status, false)
                            }
                        });
            } else {
                day_btn = day_btn.style(|theme, _status| {
                    let p = theme.extended_palette();
                    iced::widget::button::Style {
                        text_color: p.background.strong.color.scale_alpha(tokens::alpha::SUBTLE),
                        ..Default::default()
                    }
                });
            }

            week_row = week_row.push(day_btn);

            if (offset + day as usize) % 7 == 0 {
                grid = grid.push(week_row);
                week_row = row![].spacing(tokens::spacing::XXXS);
            }
        }

        // Trailing blanks for last row
        let remaining = (offset + total_days as usize) % 7;
        if remaining != 0 {
            for _ in 0..(7 - remaining) {
                week_row = week_row.push(container(text("")).width(tokens::component::replay::CALENDAR_CELL));
            }
            grid = grid.push(week_row);
        }

        let content = column![header, weekday_row, grid].spacing(tokens::spacing::XS);

        container(content)
            .width(Length::Shrink)
            .padding(tokens::spacing::SM)
            .style(style::dropdown_container)
            .into()
    }

    // ── Time Picker Popup ─────────────────────────────────────────────

    fn view_time_popup(&self) -> Element<'_, Message> {
        let (cur_h, cur_m, _) = self.parse_time();

        let h_label = container(
            text("H")
                .size(tokens::text::TINY)
                .style(palette::neutral_text)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .align_x(Alignment::Center);

        let m_label = container(
            text("M")
                .size(tokens::text::TINY)
                .style(palette::neutral_text)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .align_x(Alignment::Center);

        // Hour column
        let mut h_col = column![].spacing(tokens::spacing::XXXS);
        for h in 0..24u32 {
            let is_sel = h == cur_h;
            let btn = button(
                text(format!("{:02}", h))
                    .size(tokens::text::BODY)
                    .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([tokens::spacing::XXS, tokens::spacing::SM])
            .style(move |theme, status| {
                if is_sel {
                    style::button::primary(theme, status)
                } else {
                    style::button::transparent(theme, status, false)
                }
            })
            .on_press(Message::SelectHour(h));
            h_col = h_col.push(btn);
        }

        // Minute column (5-min increments)
        let mut m_col = column![].spacing(tokens::spacing::XXXS);
        for m in (0..60u32).step_by(5) {
            let is_sel = m == (cur_m / 5) * 5;
            let btn = button(
                text(format!("{:02}", m))
                    .size(tokens::text::BODY)
                    .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([tokens::spacing::XXS, tokens::spacing::SM])
            .style(move |theme, status| {
                if is_sel {
                    style::button::primary(theme, status)
                } else {
                    style::button::transparent(theme, status, false)
                }
            })
            .on_press(Message::SelectMinute(m));
            m_col = m_col.push(btn);
        }

        let scrollbar_cfg = scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(tokens::layout::SCROLLBAR_WIDTH)
                .scroller_width(tokens::layout::SCROLLBAR_WIDTH),
        );
        let hours = scrollable::Scrollable::with_direction(h_col, scrollbar_cfg)
            .height(Length::Fixed(160.0))
            .style(style::scroll_bar);
        let minutes = scrollable::Scrollable::with_direction(m_col, scrollbar_cfg)
            .height(Length::Fixed(160.0))
            .style(style::scroll_bar);

        let picker = row![
            column![h_label, hours]
                .spacing(tokens::spacing::XS)
                .width(84),
            column![m_label, minutes]
                .spacing(tokens::spacing::XS)
                .width(84),
        ]
        .spacing(tokens::spacing::SM);

        container(picker)
            .width(Length::Shrink)
            .padding(tokens::spacing::SM)
            .style(style::dropdown_container)
            .into()
    }

    // ── Active Replay State ───────────────────────────────────────────

    fn view_setup_active(&self) -> iced::widget::Column<'_, Message> {
        let mut col = column![].spacing(tokens::spacing::MD);

        let playback = self.playback_status;
        let status_color_fn = move |theme: &iced::Theme| match playback {
            PlaybackStatus::Playing => palette::success_color(theme),
            PlaybackStatus::Paused => palette::warning_color(theme),
            PlaybackStatus::Stopped => palette::neutral_color(theme),
        };
        let status_label = match self.playback_status {
            PlaybackStatus::Playing => "Playing",
            PlaybackStatus::Paused => "Paused",
            PlaybackStatus::Stopped => "Stopped",
        };

        let info_text = {
            let ticker = self
                .selected_stream
                .as_ref()
                .map(|s| s.ticker.to_string())
                .unwrap_or_default();
            small(format!("{} \u{00B7} {}", ticker, self.format_trade_count()))
        };

        col = col.push(
            row![
                status_badge_themed(status_color_fn, status_label),
                space::horizontal().width(Length::Fill),
                info_text,
            ]
            .align_y(Alignment::Center),
        );

        if let Some(ref stream) = self.selected_stream {
            col = col.push(small(stream.label.as_str()).style(palette::neutral_text));
        }

        let end_btn = button(
            text("End Replay")
                .size(tokens::text::BODY)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([tokens::spacing::SM, tokens::spacing::MD])
        .style(style::button::danger)
        .on_press(Message::EndReplay);

        let mut controller_btn = toolbar_icon(Icon::Replay, Message::OpenController)
            .tooltip("Open Controller")
            .padding(tokens::spacing::SM);

        if self.controller_visible {
            controller_btn = controller_btn
                .style(move |theme, status| style::button::transparent(theme, status, true));
        }

        col = col.push(
            row![end_btn, Element::<Message>::from(controller_btn)]
                .spacing(tokens::spacing::MD)
                .align_y(Alignment::Center),
        );

        col
    }
}
