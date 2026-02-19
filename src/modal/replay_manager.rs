//! Replay Manager Panel
//!
//! Sidebar popover for selecting a data stream, configuring start time,
//! and controlling historical playback (play/pause/stop, speed, seek).

use crate::component::display::status_dot::status_badge;
use crate::component::input::dropdown::DropdownBuilder;
use crate::component::input::text_field::TextFieldBuilder;
use crate::component::primitives::label::{heading, mono, small};
use crate::component::primitives::{Icon, icon_text};
use crate::style::{self, palette, tokens};
use data::feed::{DataFeedManager, FeedId, FeedProvider};
use data::state::replay_state::{PlaybackStatus, SpeedPreset};
use data::domain::TimeRange;
use data::{DateRange, FuturesTicker, FuturesTickerInfo};
use iced::widget::{button, column, container, row, rule, slider, space, text};
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
    SelectStream(StreamEntry),
    SetStartDate(String),
    SetStartTime(String),
    LoadData,
    Play,
    Pause,
    Stop,
    SetSpeed(SpeedPreset),
    Seek(f32),
    JumpForward,
    JumpBackward,
}

/// Replay manager state for the sidebar panel.
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
                if let Some(range) = downloaded_tickers.get_range_by_ticker_str(&ticker_str)
                {
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

    /// Update state from a panel message. Returns true if an engine action
    /// needs to be dispatched by the caller.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::SelectStream(entry) => {
                // Set default start date from stream's date range
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
            Message::LoadData
            | Message::Play
            | Message::Pause
            | Message::Stop
            | Message::JumpForward
            | Message::JumpBackward => {
                // These are handled by the app-level replay handler
            }
        }
    }

    /// Validate the start date string.
    fn is_date_valid(&self) -> bool {
        chrono::NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d").is_ok()
    }

    /// Validate the start time string.
    fn is_time_valid(&self) -> bool {
        chrono::NaiveTime::parse_from_str(&self.start_time, "%H:%M:%S").is_ok()
    }

    /// Format the current position as a timestamp string.
    fn format_position(&self) -> String {
        if let Some(ref range) = self.time_range {
            let elapsed_ms = self.position.saturating_sub(range.start.to_millis());
            let elapsed_secs = elapsed_ms / 1000;
            let h = elapsed_secs / 3600;
            let m = (elapsed_secs % 3600) / 60;
            let s = elapsed_secs % 60;
            format!("{:02}:{:02}:{:02}", h, m, s)
        } else {
            "00:00:00".to_string()
        }
    }

    /// Format the total duration.
    fn format_duration(&self) -> String {
        if let Some(ref range) = self.time_range {
            let total_ms =
                range.end.to_millis().saturating_sub(range.start.to_millis());
            let total_secs = total_ms / 1000;
            let h = total_secs / 3600;
            let m = (total_secs % 3600) / 60;
            let s = total_secs % 60;
            format!("{:02}:{:02}:{:02}", h, m, s)
        } else {
            "00:00:00".to_string()
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut content = column![heading("Replay"),].spacing(tokens::spacing::MD);

        // Stream selection dropdown
        let stream_dropdown = DropdownBuilder::new(
            "Data Source",
            &self.available_streams,
            self.selected_stream.clone(),
            Message::SelectStream,
        )
        .placeholder("Select a stream...")
        .width(Length::Fill);

        content = content.push(stream_dropdown);

        // Date/time inputs
        let date_field = TextFieldBuilder::new(
            "Start Date",
            "YYYY-MM-DD",
            &self.start_date,
            Message::SetStartDate,
        )
        .validate(self.start_date.is_empty() || self.is_date_valid())
        .width(Length::FillPortion(1));

        let time_field = TextFieldBuilder::new(
            "Start Time",
            "HH:MM:SS",
            &self.start_time,
            Message::SetStartTime,
        )
        .validate(self.start_time.is_empty() || self.is_time_valid())
        .width(Length::FillPortion(1));

        content = content.push(
            row![
                Element::<Message>::from(date_field),
                Element::<Message>::from(time_field),
            ]
            .spacing(tokens::spacing::MD),
        );

        // Load & Play button (or loading indicator)
        if let Some((progress, ref msg)) = self.loading_progress {
            let progress_text = small(format!("{} ({:.0}%)", msg, progress * 100.0));
            content = content.push(progress_text);
        } else if !self.data_loaded {
            let can_load = self.selected_stream.is_some()
                && (self.start_date.is_empty() || self.is_date_valid());

            let mut load_btn = button(
                text("Load & Play")
                    .size(tokens::text::BODY)
                    .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([tokens::spacing::SM, tokens::spacing::MD]);

            if can_load {
                load_btn = load_btn.on_press(Message::LoadData);
            }

            content = content.push(load_btn);
        }

        // Error display
        if let Some(ref err) = self.error {
            content = content.push(
                small(err.as_str()).color(palette::error_color()),
            );
        }

        // Playback controls (visible when data is loaded)
        if self.data_loaded {
            content = content.push(rule::horizontal(1).style(style::split_ruler));
            content = content.push(self.view_playback_controls());
        }

        container(content)
            .max_width(280.0)
            .padding(tokens::spacing::XL)
            .style(style::dashboard_modal)
            .into()
    }

    fn view_playback_controls(&self) -> Element<'_, Message> {
        // Status + info row
        let status_color = match self.playback_status {
            PlaybackStatus::Playing => palette::success_color(),
            PlaybackStatus::Paused => palette::info_color(),
            PlaybackStatus::Stopped => palette::neutral_color(),
        };
        let status_label = match self.playback_status {
            PlaybackStatus::Playing => "Playing",
            PlaybackStatus::Paused => "Paused",
            PlaybackStatus::Stopped => "Stopped",
        };

        let ticker_label = self
            .selected_stream
            .as_ref()
            .map(|s| s.ticker.to_string())
            .unwrap_or_default();

        let trade_label = if self.trade_count > 1_000_000 {
            format!("{:.1}M trades", self.trade_count as f64 / 1_000_000.0)
        } else if self.trade_count > 1_000 {
            format!("{:.1}K trades", self.trade_count as f64 / 1_000.0)
        } else {
            format!("{} trades", self.trade_count)
        };

        let status_row = row![
            status_badge(status_color, status_label),
            space::horizontal().width(Length::Fill),
            small(format!("{} · {}", ticker_label, trade_label)),
        ]
        .align_y(Alignment::Center);

        // Transport controls row
        let backward_btn = button(
            icon_text(Icon::SkipBackward, 12)
                .width(20)
                .align_x(Alignment::Center),
        )
        .on_press(Message::JumpBackward)
        .padding(tokens::spacing::XS)
        .style(|theme, status| style::button::transparent(theme, status, false));

        let play_pause_btn = match self.playback_status {
            PlaybackStatus::Playing => button(
                icon_text(Icon::Pause, 12)
                    .width(20)
                    .align_x(Alignment::Center),
            )
            .on_press(Message::Pause)
            .padding(tokens::spacing::XS)
            .style(|theme, status| style::button::transparent(theme, status, false)),
            _ => button(
                icon_text(Icon::Play, 12)
                    .width(20)
                    .align_x(Alignment::Center),
            )
            .on_press(Message::Play)
            .padding(tokens::spacing::XS)
            .style(|theme, status| style::button::transparent(theme, status, false)),
        };

        let stop_btn = button(
            icon_text(Icon::Stop, 12)
                .width(20)
                .align_x(Alignment::Center),
        )
        .on_press(Message::Stop)
        .padding(tokens::spacing::XS)
        .style(|theme, status| style::button::transparent(theme, status, false));

        let forward_btn = button(
            icon_text(Icon::SkipForward, 12)
                .width(20)
                .align_x(Alignment::Center),
        )
        .on_press(Message::JumpForward)
        .padding(tokens::spacing::XS)
        .style(|theme, status| style::button::transparent(theme, status, false));

        // Speed selector
        let speeds: Vec<SpeedPreset> = SpeedPreset::all_presets();
        let speed_picker = iced::widget::pick_list(
            speeds,
            Some(self.speed),
            Message::SetSpeed,
        )
        .text_size(tokens::text::SMALL);

        let transport_row = row![
            backward_btn,
            play_pause_btn,
            stop_btn,
            forward_btn,
            space::horizontal().width(Length::Fill),
            speed_picker,
        ]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center);

        // Seek slider
        let seek_slider = slider(0.0..=1.0, self.progress, Message::Seek).step(0.001);

        // Position / Duration
        let position_text = row![
            mono(self.format_position()),
            small(" / "),
            mono(self.format_duration()),
        ]
        .align_y(Alignment::Center);

        // Progress percentage
        let progress_pct = small(format!("{:.0}%", self.progress * 100.0));

        let seek_row = row![
            seek_slider,
            progress_pct,
        ]
        .spacing(tokens::spacing::SM)
        .align_y(Alignment::Center);

        column![status_row, transport_row, seek_row, position_text,]
            .spacing(tokens::spacing::MD)
            .into()
    }
}
