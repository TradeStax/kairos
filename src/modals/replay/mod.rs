//! Replay Manager
//!
//! Two-component replay UI:
//! 1. **Setup modal** – sidebar popover for stream/date selection and replay lifecycle
//! 2. **Floating controller** – compact draggable playback panel with volume trackbar

mod controller_view;
mod setup_view;

use chrono::{Datelike, NaiveDate};
use data::domain::TimeRange;
use data::feed::{DataFeedManager, FeedId, FeedProvider};
use data::services::VolumeBucket;
use data::state::replay::{PlaybackStatus, SpeedPreset};
use data::{DateRange, FuturesTicker, FuturesTickerInfo, UserTimezone};

// ── Layout offset constants ───────────────────────────────────────────
// Pre-calculated Y offsets to position popups *below* their trigger buttons.
// Accounts for modal padding(12), line heights (1.3x font size), and spacing.
// modal_pad(12) + title(18) + MD(8) + label(17) + XS(4) + btn(28) = 87 + gap
pub(super) const STREAM_POPUP_Y: f32 = 90.0;
// stream(87) + MD(8) + label(17) + XS(4) + btn(28) = 144 + gap
pub(super) const DATETIME_POPUP_Y: f32 = 148.0;

// Square calendar day cell size
pub(super) const CALENDAR_CELL: f32 = 26.0;

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
    pub(super) fn display_name(&self, max_len: usize) -> String {
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

    pub(super) fn parse_time(&self) -> (u32, u32, u32) {
        let parts: Vec<&str> = self.start_time.split(':').collect();
        let h = parts.first().and_then(|s| s.parse().ok()).unwrap_or(9);
        let m = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(30);
        let s = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        (h, m, s)
    }

    pub(super) fn is_date_valid(&self) -> bool {
        NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d").is_ok()
    }

    pub(super) fn format_position(&self, timezone: UserTimezone) -> String {
        if self.position > 0 {
            return timezone.format_replay_timestamp(self.position as i64);
        }
        if let Some(ref range) = self.time_range {
            return timezone.format_replay_timestamp(range.start.to_millis() as i64);
        }
        "--:--:--".to_string()
    }

    pub(super) fn format_end_time(&self, timezone: UserTimezone) -> String {
        if let Some(ref range) = self.time_range {
            timezone.format_replay_timestamp(range.end.to_millis() as i64)
        } else {
            "--:--:--".to_string()
        }
    }

    pub(super) fn format_trade_count(&self) -> String {
        if self.trade_count > 1_000_000 {
            format!("{:.1}M trades", self.trade_count as f64 / 1_000_000.0)
        } else if self.trade_count > 1_000 {
            format!("{:.1}K trades", self.trade_count as f64 / 1_000.0)
        } else {
            format!("{} trades", self.trade_count)
        }
    }
}

// ── Free Functions ────────────────────────────────────────────────────

pub(super) fn first_of_month(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap()
}

pub(super) fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.unwrap()
        .signed_duration_since(NaiveDate::from_ymd_opt(year, month, 1).unwrap())
        .num_days() as u32
}
