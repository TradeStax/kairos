//! Replay Manager
//!
//! Two-component replay UI:
//! 1. **Setup modal** – sidebar popover for stream/date selection and replay lifecycle
//! 2. **Floating controller** – compact draggable playback panel with volume trackbar

use crate::component::display::progress_bar::ProgressBarBuilder;
use crate::component::display::status_dot::status_badge;
use crate::component::input::dropdown::DropdownBuilder;
use crate::component::input::text_field::TextFieldBuilder;
use crate::component::primitives::Icon;
use crate::component::primitives::icon_button::toolbar_icon;
use crate::component::primitives::label::{mono, small, title};
use crate::style::{self, palette, tokens};
use crate::component::input::volume_trackbar::volume_trackbar;
use data::domain::TimeRange;
use data::feed::{DataFeedManager, FeedId, FeedProvider};
use data::services::VolumeBucket;
use data::state::replay_state::{PlaybackStatus, SpeedPreset};
use data::{DateRange, FuturesTicker, FuturesTickerInfo, UserTimezone};
use iced::mouse;
use iced::widget::{button, column, container, mouse_area, pick_list, row, space, text};
use iced::{Alignment, Element, Length};

/// Entry representing a connected data stream available for replay.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamEntry {
    pub feed_id: FeedId,
    pub ticker: FuturesTicker,
    pub ticker_info: FuturesTickerInfo,
    pub date_range: DateRange,
    pub provider: FeedProvider,
    pub label: String,
}

impl std::fmt::Display for StreamEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    // Setup messages
    SelectStream(StreamEntry),
    SetStartDate(String),
    SetStartTime(String),
    LoadData,
    // Playback control
    Play,
    Pause,
    SetSpeed(SpeedPreset),
    Seek(f32),
    JumpForward,
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

    // Start time selection
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

    /// Refresh available streams from connected feeds and downloaded tickers.
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
            .filter(|f| f.status.is_connected())
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
                            label,
                        });
                    }
                }
            }
        }

        // If previously selected stream is no longer available, clear selection
        if let Some(ref selected) = self.selected_stream {
            if !self.available_streams.contains(selected) {
                self.selected_stream = None;
            }
        }
    }

    /// Update state from a panel message.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::SelectStream(entry) => {
                self.start_date = entry.date_range.start.format("%Y-%m-%d").to_string();
                self.selected_stream = Some(entry);
                self.error = None;
            }
            Message::SetStartDate(date) => {
                self.start_date = date;
            }
            Message::SetStartTime(time) => {
                self.start_time = time;
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
            // Handled by app-level replay handler
            Message::LoadData
            | Message::Play
            | Message::Pause
            | Message::EndReplay
            | Message::JumpForward
            | Message::JumpBackward => {}
        }
    }

    // ── Helpers ────────────────────────────────────────────────────────

    fn is_date_valid(&self) -> bool {
        chrono::NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d").is_ok()
    }

    fn is_time_valid(&self) -> bool {
        chrono::NaiveTime::parse_from_str(&self.start_time, "%H:%M:%S").is_ok()
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

    /// Sidebar popover content: setup form or active-replay controls.
    pub fn view_setup_modal(&self, _timezone: UserTimezone) -> Element<'_, Message> {
        let content = if self.data_loaded {
            self.view_setup_active()
        } else {
            self.view_setup_form()
        };

        container(content)
            .width(Length::Fixed(280.0))
            .padding(tokens::spacing::LG)
            .style(style::dashboard_modal)
            .into()
    }

    /// Setup form: stream dropdown, date/time, start button.
    fn view_setup_form(&self) -> iced::widget::Column<'_, Message> {
        let mut col = column![title("Replay")].spacing(tokens::spacing::MD);

        // Stream dropdown
        let stream_dropdown = DropdownBuilder::new(
            "Data Source",
            &self.available_streams,
            self.selected_stream.clone(),
            Message::SelectStream,
        )
        .placeholder("Select a stream...")
        .width(Length::Fill);
        col = col.push(stream_dropdown);

        // Date/time row
        let date_field = TextFieldBuilder::new(
            "Date",
            "YYYY-MM-DD",
            &self.start_date,
            Message::SetStartDate,
        )
        .validate(self.start_date.is_empty() || self.is_date_valid())
        .width(Length::FillPortion(1));

        let time_field =
            TextFieldBuilder::new("Time", "HH:MM:SS", &self.start_time, Message::SetStartTime)
                .validate(self.start_time.is_empty() || self.is_time_valid())
                .width(Length::FillPortion(1));

        col = col.push(
            row![
                Element::<Message>::from(date_field),
                Element::<Message>::from(time_field),
            ]
            .spacing(tokens::spacing::MD),
        );

        // Loading progress or start button
        if let Some((progress, ref msg)) = self.loading_progress {
            let bar: Element<'_, Message> = ProgressBarBuilder::new(progress, 1.0)
                .label(msg)
                .show_percentage(true)
                .girth(4.0)
                .into();
            col = col.push(bar);
        } else {
            let can_load = self.selected_stream.is_some()
                && (self.start_date.is_empty() || self.is_date_valid());

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

        // Error
        if let Some(ref err) = self.error {
            col = col.push(small(err.as_str()).color(palette::error_color()));
        }

        col
    }

    /// Active replay state: disabled info, End Replay + Open Controller buttons.
    fn view_setup_active(&self) -> iced::widget::Column<'_, Message> {
        let mut col = column![title("Replay")].spacing(tokens::spacing::MD);

        // Status row
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
            small(format!("{} · {}", ticker, self.format_trade_count()))
        };

        col = col.push(
            row![
                status_badge(status_color, status_label),
                space::horizontal().width(Length::Fill),
                info_text,
            ]
            .align_y(Alignment::Center),
        );

        // Stream info (disabled appearance)
        if let Some(ref stream) = self.selected_stream {
            col = col.push(small(stream.label.as_str()).color(palette::neutral_color()));
        }

        // Button row: End Replay (danger, fill) + Open Controller (icon)
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
        // Title bar: info + close button, draggable
        let info_label = {
            let ticker = self
                .selected_stream
                .as_ref()
                .map(|s| s.ticker.to_string())
                .unwrap_or_default();
            small(format!("{} · {}", ticker, self.format_trade_count()))
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

        // Play/pause button: circular style
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

        // Volume trackbar
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

        // Footer: position, speed, duration
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
