//! Replay Manager
//!
//! Two-component replay UI:
//! 1. **Setup modal** – sidebar popover for stream/date selection and replay lifecycle
//! 2. **Floating controller** – compact draggable playback panel with volume trackbar

use crate::components::display::progress_bar::ProgressBarBuilder;
use crate::components::display::status_dot::status_badge;
use crate::components::input::volume_trackbar::volume_trackbar;
use crate::components::primitives::icon_button::toolbar_icon;
use crate::components::primitives::label::{mono, small, title};
use crate::components::primitives::{Icon, icon_text};
use crate::style::{self, palette, tokens};
use chrono::{Datelike, NaiveDate, Weekday};
use data::domain::TimeRange;
use data::feed::{DataFeedManager, FeedId, FeedProvider};
use data::services::VolumeBucket;
use data::state::replay::{PlaybackStatus, SpeedPreset};
use data::{DateRange, FuturesTicker, FuturesTickerInfo, UserTimezone};
use iced::mouse;
use iced::widget::{
    button, column, container, mouse_area, opaque, pick_list, row, scrollable, space, stack, text,
};
use iced::{Alignment, Element, Length, Padding};

// ── Layout offset constants ───────────────────────────────────────────
// Pre-calculated Y offsets to position popups *below* their trigger buttons.
// Accounts for modal padding(12), line heights (1.3x font size), and spacing.
// modal_pad(12) + title(18) + MD(8) + label(17) + XS(4) + btn(28) = 87 + gap
const STREAM_POPUP_Y: f32 = 90.0;
// stream(87) + MD(8) + label(17) + XS(4) + btn(28) = 144 + gap
const DATETIME_POPUP_Y: f32 = 148.0;

// Square calendar day cell size
const CALENDAR_CELL: f32 = 26.0;

/// Which popup is currently open (only one at a time).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Popup {
    StreamPicker,
    DatePicker,
    TimePicker,
}

/// Entry representing a connected data stream available for replay.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamEntry {
    pub feed_id: FeedId,
    pub ticker: FuturesTicker,
    pub ticker_info: FuturesTickerInfo,
    pub date_range: DateRange,
    pub provider: FeedProvider,
    pub feed_name: String,
    pub label: String,
}

impl StreamEntry {
    /// Feed name truncated to fit compact dropdown items.
    fn display_name(&self, max_len: usize) -> String {
        if self.feed_name.len() <= max_len {
            self.feed_name.clone()
        } else {
            let mut s = self.feed_name[..max_len.saturating_sub(1)].to_string();
            s.push('\u{2026}');
            s
        }
    }
}

impl std::fmt::Display for StreamEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    // Popup control
    TogglePopup(Popup),
    ClosePopups,
    // Stream selection
    SelectStream(StreamEntry),
    // Calendar navigation & date selection
    CalendarPrevMonth,
    CalendarNextMonth,
    SelectDate(NaiveDate),
    // Time selection
    SelectHour(u32),
    SelectMinute(u32),
    // Data loading
    LoadData,
    // Playback control
    Play,
    Pause,
    SetSpeed(SpeedPreset),
    Seek(f32),
    #[allow(dead_code)] // Planned: skip forward in replay
    JumpForward,
    #[allow(dead_code)] // Planned: skip backward in replay
    JumpBackward,
    // Lifecycle
    EndReplay,
    OpenController,
    CloseController,
    // Volume histogram
    VolumeHistogramReady(Vec<VolumeBucket>),
    // Drag (floating controller)
    DragStart,
    DragMove(iced::Point),
    DragEnd,
}

/// Replay manager state.
pub struct ReplayManager {
    // Feed/stream selection
    pub selected_stream: Option<StreamEntry>,
    pub available_streams: Vec<StreamEntry>,

    // Popup state
    pub active_popup: Option<Popup>,
    pub calendar_month: Option<NaiveDate>,

    // Start time selection (string format for backward compat with replay engine)
    pub start_date: String,
    pub start_time: String,

    // Playback state (mirrored from engine for UI)
    pub playback_status: PlaybackStatus,
    pub speed: SpeedPreset,
    pub position: u64,
    pub progress: f32,
    pub time_range: Option<TimeRange>,

    // Data state
    pub loading_progress: Option<(f32, String)>,
    pub trade_count: usize,
    pub depth_count: usize,
    pub data_loaded: bool,
    pub error: Option<String>,

    // Volume histogram for the trackbar
    pub volume_buckets: Vec<VolumeBucket>,

    // Floating controller state
    pub controller_visible: bool,
    pub panel_position: iced::Point,
    pub is_dragging: bool,
    drag_offset: iced::Vector,
}

impl ReplayManager {
    pub fn new() -> Self {
        Self {
            selected_stream: None,
            available_streams: Vec::new(),
            active_popup: None,
            calendar_month: None,
            start_date: String::new(),
            start_time: "09:30:00".to_string(),
            playback_status: PlaybackStatus::Stopped,
            speed: SpeedPreset::Normal,
            position: 0,
            progress: 0.0,
            time_range: None,
            loading_progress: None,
            trade_count: 0,
            depth_count: 0,
            data_loaded: false,
            error: None,
            volume_buckets: Vec::new(),
            controller_visible: false,
            panel_position: iced::Point::new(100.0, 100.0),
            is_dragging: false,
            drag_offset: iced::Vector::new(0.0, 0.0),
        }
    }

    /// Refresh available streams from connected historical feeds
    /// and downloaded tickers.
    pub fn refresh_streams(
        &mut self,
        feed_manager: &DataFeedManager,
        downloaded_tickers: &data::DownloadedTickersRegistry,
        ticker_infos: &std::collections::HashMap<String, FuturesTickerInfo>,
    ) {
        self.available_streams.clear();

        for feed in feed_manager
            .feeds()
            .iter()
            .filter(|f| f.status.is_connected() && !f.is_realtime())
        {
            for ticker_str in downloaded_tickers.list_tickers() {
                if let Some(range) = downloaded_tickers.get_range_by_ticker_str(&ticker_str) {
                    if let Some(info) = ticker_infos.get(&ticker_str) {
                        let label = format!(
                            "{} {} {}-{}",
                            ticker_str,
                            feed.provider.display_name(),
                            range.start.format("%m/%d"),
                            range.end.format("%m/%d"),
                        );
                        self.available_streams.push(StreamEntry {
                            feed_id: feed.id,
                            ticker: info.ticker,
                            ticker_info: *info,
                            date_range: range,
                            provider: feed.provider,
                            feed_name: feed.name.clone(),
                            label,
                        });
                    }
                }
            }
        }

        if let Some(ref selected) = self.selected_stream {
            if !self.available_streams.contains(selected) {
                self.selected_stream = None;
            }
        }
    }

    /// Update state from a panel message.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::TogglePopup(popup) => {
                if self.active_popup == Some(popup) {
                    self.active_popup = None;
                } else {
                    self.active_popup = Some(popup);
                    if popup == Popup::DatePicker {
                        self.init_calendar_month();
                    }
                }
            }
            Message::ClosePopups => {
                self.active_popup = None;
            }
            Message::SelectStream(entry) => {
                self.start_date = entry.date_range.start.format("%Y-%m-%d").to_string();
                self.calendar_month = Some(first_of_month(entry.date_range.start));
                self.selected_stream = Some(entry);
                self.active_popup = None;
                self.error = None;
            }
            Message::CalendarPrevMonth => {
                if let Some(ref mut m) = self.calendar_month {
                    *m = *m - chrono::Months::new(1);
                }
            }
            Message::CalendarNextMonth => {
                if let Some(ref mut m) = self.calendar_month {
                    *m = *m + chrono::Months::new(1);
                }
            }
            Message::SelectDate(date) => {
                self.start_date = date.format("%Y-%m-%d").to_string();
                self.active_popup = None;
            }
            Message::SelectHour(h) => {
                let (_, m, s) = self.parse_time();
                self.start_time = format!("{:02}:{:02}:{:02}", h, m, s);
            }
            Message::SelectMinute(m) => {
                let (h, _, s) = self.parse_time();
                self.start_time = format!("{:02}:{:02}:{:02}", h, m, s);
            }
            Message::SetSpeed(speed) => {
                self.speed = speed;
            }
            Message::Seek(progress) => {
                self.progress = progress;
            }
            Message::OpenController => {
                self.controller_visible = true;
            }
            Message::CloseController => {
                self.controller_visible = false;
            }
            Message::VolumeHistogramReady(buckets) => {
                self.volume_buckets = buckets;
            }
            Message::DragStart => {
                self.is_dragging = true;
                self.drag_offset = iced::Vector::ZERO;
            }
            Message::DragMove(cursor_pos) => {
                if self.is_dragging {
                    if self.drag_offset == iced::Vector::ZERO {
                        self.drag_offset = iced::Vector::new(
                            cursor_pos.x - self.panel_position.x,
                            cursor_pos.y - self.panel_position.y,
                        );
                    }
                    self.panel_position = iced::Point::new(
                        (cursor_pos.x - self.drag_offset.x).max(0.0),
                        (cursor_pos.y - self.drag_offset.y).max(0.0),
                    );
                }
            }
            Message::DragEnd => {
                self.is_dragging = false;
            }
            Message::LoadData
            | Message::Play
            | Message::Pause
            | Message::EndReplay
            | Message::JumpForward
            | Message::JumpBackward => {}
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────

    fn init_calendar_month(&mut self) {
        if self.calendar_month.is_some() {
            return;
        }
        if let Ok(d) = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d") {
            self.calendar_month = Some(first_of_month(d));
        } else if let Some(ref s) = self.selected_stream {
            self.calendar_month = Some(first_of_month(s.date_range.start));
        } else {
            self.calendar_month = Some(first_of_month(chrono::Utc::now().date_naive()));
        }
    }

    fn parse_time(&self) -> (u32, u32, u32) {
        let parts: Vec<&str> = self.start_time.split(':').collect();
        let h = parts.first().and_then(|s| s.parse().ok()).unwrap_or(9);
        let m = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(30);
        let s = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        (h, m, s)
    }

    fn is_date_valid(&self) -> bool {
        NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d").is_ok()
    }

    fn format_position(&self, timezone: UserTimezone) -> String {
        if self.position > 0 {
            return timezone.format_replay_timestamp(self.position as i64);
        }
        if let Some(ref range) = self.time_range {
            return timezone.format_replay_timestamp(range.start.to_millis() as i64);
        }
        "--:--:--".to_string()
    }

    fn format_end_time(&self, timezone: UserTimezone) -> String {
        if let Some(ref range) = self.time_range {
            timezone.format_replay_timestamp(range.end.to_millis() as i64)
        } else {
            "--:--:--".to_string()
        }
    }

    fn format_trade_count(&self) -> String {
        if self.trade_count > 1_000_000 {
            format!("{:.1}M trades", self.trade_count as f64 / 1_000_000.0)
        } else if self.trade_count > 1_000 {
            format!("{:.1}K trades", self.trade_count as f64 / 1_000.0)
        } else {
            format!("{} trades", self.trade_count)
        }
    }

    // ── Sidebar Setup Modal ───────────────────────────────────────────

    /// Sidebar popover: setup form or active-replay controls.
    pub fn view_setup_modal(&self, _timezone: UserTimezone) -> Element<'_, Message> {
        let form = if self.data_loaded {
            self.view_setup_active()
        } else {
            self.view_setup_form()
        };

        let base = container(form)
            .width(Length::Fixed(280.0))
            .padding(tokens::spacing::LG)
            .style(style::dashboard_modal);

        if let Some(popup) = self.active_popup {
            let (popup_content, offset_y) = match popup {
                Popup::StreamPicker => (self.view_stream_popup(), STREAM_POPUP_Y),
                Popup::DatePicker => (self.view_date_popup(), DATETIME_POPUP_Y),
                Popup::TimePicker => (self.view_time_popup(), DATETIME_POPUP_Y),
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
        let mut col = column![title("Replay")].spacing(tokens::spacing::MD);

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
            col = col.push(small(err.as_str()).color(palette::error_color()));
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
                .color(palette::neutral_color())
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
                        .color(palette::neutral_color()),
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
                .color(palette::neutral_color());

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
                            .color(palette::neutral_color())
                            .align_x(Alignment::Center),
                    )
                    .width(CALENDAR_CELL),
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
            week_row = week_row.push(container(text("")).width(CALENDAR_CELL));
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

            let mut day_btn = button(day_text).width(CALENDAR_CELL).padding([7.0, 0.0]);

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
                week_row = week_row.push(container(text("")).width(CALENDAR_CELL));
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
                .color(palette::neutral_color())
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .align_x(Alignment::Center);

        let m_label = container(
            text("M")
                .size(tokens::text::TINY)
                .color(palette::neutral_color())
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
        let mut col = column![title("Replay")].spacing(tokens::spacing::MD);

        let status_color = match self.playback_status {
            PlaybackStatus::Playing => palette::success_color(),
            PlaybackStatus::Paused => palette::warning_color(),
            PlaybackStatus::Stopped => palette::neutral_color(),
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
                status_badge(status_color, status_label),
                space::horizontal().width(Length::Fill),
                info_text,
            ]
            .align_y(Alignment::Center),
        );

        if let Some(ref stream) = self.selected_stream {
            col = col.push(small(stream.label.as_str()).color(palette::neutral_color()));
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

    // ── Floating Controller ───────────────────────────────────────────

    /// Compact floating controller with volume trackbar.
    pub fn view_floating_controller(&self, timezone: UserTimezone) -> Element<'_, Message> {
        let info_label = {
            let ticker = self
                .selected_stream
                .as_ref()
                .map(|s| s.ticker.to_string())
                .unwrap_or_default();
            small(format!("{} \u{00B7} {}", ticker, self.format_trade_count()))
        };

        let close_btn = button(text("\u{00D7}").size(tokens::text::TITLE))
            .on_press(Message::CloseController)
            .style(|theme, status| style::button::transparent(theme, status, false))
            .padding([0.0, tokens::spacing::XS]);

        let title_row = row![
            info_label,
            space::horizontal().width(Length::Fill),
            close_btn,
        ]
        .align_y(Alignment::Center);

        let title_bar = mouse_area(
            container(title_row)
                .width(Length::Fill)
                .padding([tokens::spacing::XS, tokens::spacing::MD])
                .style(style::floating_panel_header),
        )
        .on_press(Message::DragStart)
        .interaction(mouse::Interaction::Grab);

        let play_pause: Element<'_, Message> = match self.playback_status {
            PlaybackStatus::Playing => toolbar_icon(Icon::Pause, Message::Pause)
                .size(12)
                .style(|theme, status| {
                    let mut s = style::button::transparent(theme, status, false);
                    s.border.radius = tokens::radius::ROUND.into();
                    s
                })
                .tooltip("Pause")
                .into(),
            _ => toolbar_icon(Icon::Play, Message::Play)
                .size(12)
                .style(|theme, status| {
                    let mut s = style::button::transparent(theme, status, false);
                    s.border.radius = tokens::radius::ROUND.into();
                    s
                })
                .tooltip("Play")
                .into(),
        };

        let trackbar = volume_trackbar(
            &self.volume_buckets,
            self.progress,
            self.time_range.as_ref(),
            timezone,
            Message::Seek,
        );

        let main_row = row![play_pause, trackbar]
            .spacing(tokens::spacing::SM)
            .align_y(Alignment::Center);

        let speed_picker = pick_list(
            &[
                SpeedPreset::Quarter,
                SpeedPreset::Half,
                SpeedPreset::Normal,
                SpeedPreset::Double,
                SpeedPreset::Five,
                SpeedPreset::Ten,
            ][..],
            Some(self.speed),
            Message::SetSpeed,
        )
        .text_size(tokens::text::TINY)
        .padding([tokens::spacing::XXS, tokens::spacing::XS]);

        let footer = row![
            mono(self.format_position(timezone)),
            space::horizontal().width(Length::Fill),
            speed_picker,
            space::horizontal().width(Length::Fill),
            mono(self.format_end_time(timezone)),
        ]
        .align_y(Alignment::Center);

        let body = column![main_row, footer]
            .spacing(tokens::spacing::XS)
            .padding([tokens::spacing::XS, tokens::spacing::MD]);

        let content = column![title_bar, body];

        container(content)
            .width(Length::Fixed(450.0))
            .style(style::floating_panel)
            .into()
    }
}

// ── Free Functions ────────────────────────────────────────────────────

fn first_of_month(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap()
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.unwrap()
        .signed_duration_since(NaiveDate::from_ymd_opt(year, month, 1).unwrap())
        .num_days() as u32
}
