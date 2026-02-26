//! Replay State and Data types

use crate::domain::core::types::{TimeRange, Timestamp};
use crate::domain::instrument::futures::{FuturesTicker, FuturesTickerInfo};
use crate::domain::market::entities::{Candle, Depth, Trade};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Playback status for the replay engine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlaybackStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Speed preset for replay
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum SpeedPreset {
    Quarter,
    Half,
    #[default]
    Normal,
    Double,
    Five,
    Ten,
    TwentyFive,
    Fifty,
    Hundred,
    Custom(f32),
}

impl SpeedPreset {
    pub fn to_multiplier(&self) -> f32 {
        match self {
            SpeedPreset::Quarter => 0.25,
            SpeedPreset::Half => 0.5,
            SpeedPreset::Normal => 1.0,
            SpeedPreset::Double => 2.0,
            SpeedPreset::Five => 5.0,
            SpeedPreset::Ten => 10.0,
            SpeedPreset::TwentyFive => 25.0,
            SpeedPreset::Fifty => 50.0,
            SpeedPreset::Hundred => 100.0,
            SpeedPreset::Custom(speed) => *speed,
        }
    }

    pub fn from_multiplier(speed: f32) -> Self {
        match speed {
            s if (s - 0.25).abs() < 0.01 => SpeedPreset::Quarter,
            s if (s - 0.5).abs() < 0.01 => SpeedPreset::Half,
            s if (s - 1.0).abs() < 0.01 => SpeedPreset::Normal,
            s if (s - 2.0).abs() < 0.01 => SpeedPreset::Double,
            s if (s - 5.0).abs() < 0.01 => SpeedPreset::Five,
            s if (s - 10.0).abs() < 0.01 => SpeedPreset::Ten,
            s if (s - 25.0).abs() < 0.01 => SpeedPreset::TwentyFive,
            s if (s - 50.0).abs() < 0.01 => SpeedPreset::Fifty,
            s if (s - 100.0).abs() < 0.01 => SpeedPreset::Hundred,
            s => SpeedPreset::Custom(s),
        }
    }

    pub fn label(&self) -> String {
        match self {
            SpeedPreset::Quarter => "0.25x".to_string(),
            SpeedPreset::Half => "0.5x".to_string(),
            SpeedPreset::Normal => "1x".to_string(),
            SpeedPreset::Double => "2x".to_string(),
            SpeedPreset::Five => "5x".to_string(),
            SpeedPreset::Ten => "10x".to_string(),
            SpeedPreset::TwentyFive => "25x".to_string(),
            SpeedPreset::Fifty => "50x".to_string(),
            SpeedPreset::Hundred => "100x".to_string(),
            SpeedPreset::Custom(s) => format!("{:.1}x", s),
        }
    }

    pub fn all_presets() -> Vec<SpeedPreset> {
        vec![
            SpeedPreset::Quarter,
            SpeedPreset::Half,
            SpeedPreset::Normal,
            SpeedPreset::Double,
            SpeedPreset::Five,
            SpeedPreset::Ten,
            SpeedPreset::TwentyFive,
            SpeedPreset::Fifty,
            SpeedPreset::Hundred,
        ]
    }
}

impl std::fmt::Display for SpeedPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Replay state for managing playback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayState {
    pub status: PlaybackStatus,
    pub speed: SpeedPreset,
    pub position: u64,
    pub time_range: TimeRange,
    pub ticker_info: Option<FuturesTickerInfo>,
    pub is_loaded: bool,
    pub trade_count: usize,
    pub depth_count: usize,
}

impl ReplayState {
    pub fn new() -> Self {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        Self {
            status: PlaybackStatus::Stopped,
            speed: SpeedPreset::Normal,
            position: now,
            time_range: TimeRange::new(Timestamp::from_millis(now), Timestamp::from_millis(now))
                .expect("invariant: equal timestamps are valid"),
            ticker_info: None,
            is_loaded: false,
            trade_count: 0,
            depth_count: 0,
        }
    }

    pub fn with_data(
        time_range: TimeRange,
        ticker_info: FuturesTickerInfo,
        trade_count: usize,
        depth_count: usize,
    ) -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            speed: SpeedPreset::Normal,
            position: time_range.start.to_millis(),
            time_range,
            ticker_info: Some(ticker_info),
            is_loaded: true,
            trade_count,
            depth_count,
        }
    }

    pub fn reset(&mut self) {
        self.status = PlaybackStatus::Stopped;
        self.position = self.time_range.start.to_millis();
    }

    pub fn clear(&mut self) {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        self.status = PlaybackStatus::Stopped;
        self.position = now;
        self.time_range = TimeRange::new(Timestamp::from_millis(now), Timestamp::from_millis(now))
            .expect("invariant: equal timestamps are valid");
        self.ticker_info = None;
        self.is_loaded = false;
        self.trade_count = 0;
        self.depth_count = 0;
    }

    pub fn progress(&self) -> f32 {
        let start = self.time_range.start.to_millis();
        let end = self.time_range.end.to_millis();
        if end <= start {
            return 0.0;
        }
        let elapsed = self.position.saturating_sub(start) as f32;
        let total = (end - start) as f32;
        (elapsed / total).clamp(0.0, 1.0)
    }

    pub fn is_at_end(&self) -> bool {
        self.position >= self.time_range.end.to_millis()
    }

    pub fn is_at_start(&self) -> bool {
        self.position <= self.time_range.start.to_millis()
    }

    pub fn format_position(&self) -> String {
        let dt = chrono::DateTime::from_timestamp_millis(self.position as i64)
            .unwrap_or_else(chrono::Utc::now);
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    pub fn format_elapsed(&self) -> String {
        let elapsed_ms = self
            .position
            .saturating_sub(self.time_range.start.to_millis());
        let elapsed_secs = elapsed_ms / 1000;
        let hours = elapsed_secs / 3600;
        let minutes = (elapsed_secs % 3600) / 60;
        let seconds = elapsed_secs % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }

    pub fn format_duration(&self) -> String {
        let total_ms = self.time_range.end.to_millis() - self.time_range.start.to_millis();
        let total_secs = total_ms / 1000;
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }

    pub fn data_summary(&self) -> String {
        if !self.is_loaded {
            return "No data loaded".to_string();
        }
        let ticker = self
            .ticker_info
            .as_ref()
            .map(|ti| ti.ticker.to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        format!(
            "{}: {} trades, {} depth snapshots",
            ticker, self.trade_count, self.depth_count
        )
    }
}

impl Default for ReplayState {
    fn default() -> Self {
        Self::new()
    }
}

/// Replay data container for efficient time-based access
#[derive(Debug, Clone)]
pub struct ReplayData {
    pub trades: BTreeMap<u64, Vec<Trade>>,
    pub depth_snapshots: BTreeMap<u64, Depth>,
    pub candles_cache: BTreeMap<String, Vec<Candle>>,
    pub ticker_info: FuturesTickerInfo,
    pub time_range: TimeRange,
}

impl ReplayData {
    pub fn new(
        trades: Vec<Trade>,
        depth_snapshots: Vec<Depth>,
        ticker_info: FuturesTickerInfo,
    ) -> Self {
        let mut trades_map = BTreeMap::new();
        for trade in trades {
            trades_map
                .entry(trade.time.to_millis())
                .or_insert_with(Vec::new)
                .push(trade);
        }

        let mut depth_map = BTreeMap::new();
        for snapshot in depth_snapshots {
            depth_map.insert(snapshot.time, snapshot);
        }

        let start = trades_map
            .keys()
            .next()
            .copied()
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis() as u64);
        let end = trades_map.keys().last().copied().unwrap_or(start);

        let time_range = TimeRange::new(Timestamp::from_millis(start), Timestamp::from_millis(end))
            .expect("invariant: start <= end from BTreeMap keys");

        Self {
            trades: trades_map,
            depth_snapshots: depth_map,
            candles_cache: BTreeMap::new(),
            ticker_info,
            time_range,
        }
    }

    pub fn trades_in_window(&self, start: u64, end: u64) -> Vec<Trade> {
        self.trades
            .range(start..=end)
            .flat_map(|(_, trades)| trades.iter().cloned())
            .collect()
    }

    pub fn trades_after(&self, after: u64, up_to: u64) -> Vec<Trade> {
        use std::ops::Bound;
        self.trades
            .range((Bound::Excluded(after), Bound::Included(up_to)))
            .flat_map(|(_, trades)| trades.iter().cloned())
            .collect()
    }

    pub fn depth_at(&self, timestamp: u64) -> Option<&Depth> {
        self.depth_snapshots
            .range(..=timestamp)
            .last()
            .map(|(_, snapshot)| snapshot)
    }

    pub fn next_trades(&self, after: u64, limit: usize) -> Vec<Trade> {
        self.trades
            .range((after + 1)..)
            .take(limit)
            .flat_map(|(_, trades)| trades.iter().cloned())
            .collect()
    }

    pub fn stats(&self) -> ReplayDataStats {
        let total_trades: usize = self.trades.values().map(|v| v.len()).sum();
        let total_depth = self.depth_snapshots.len();

        ReplayDataStats {
            trade_count: total_trades,
            depth_count: total_depth,
            time_range: self.time_range,
            ticker: self.ticker_info.ticker,
        }
    }

    pub fn memory_usage(&self) -> usize {
        let trades_size = self
            .trades
            .values()
            .map(|v| v.len() * std::mem::size_of::<Trade>())
            .sum::<usize>();
        let depth_size = self.depth_snapshots.len() * std::mem::size_of::<Depth>();
        let candles_size = self
            .candles_cache
            .values()
            .map(|v| v.len() * std::mem::size_of::<Candle>())
            .sum::<usize>();
        trades_size + depth_size + candles_size
    }

    pub fn clear_candles_cache(&mut self) {
        self.candles_cache.clear();
    }

    pub fn cache_candles(&mut self, timeframe: String, candles: Vec<Candle>) {
        self.candles_cache.insert(timeframe, candles);
    }

    pub fn get_cached_candles(&self, timeframe: &str) -> Option<&Vec<Candle>> {
        self.candles_cache.get(timeframe)
    }
}

/// Statistics for replay data
#[derive(Debug, Clone)]
pub struct ReplayDataStats {
    pub trade_count: usize,
    pub depth_count: usize,
    pub time_range: TimeRange,
    pub ticker: FuturesTicker,
}

impl ReplayDataStats {
    pub fn summary(&self) -> String {
        format!(
            "{}: {} trades, {} depth snapshots, {} to {}",
            self.ticker,
            self.trade_count,
            self.depth_count,
            self.time_range.start.to_datetime().format("%Y-%m-%d %H:%M"),
            self.time_range.end.to_datetime().format("%Y-%m-%d %H:%M")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FuturesVenue, Price, Quantity, Side};

    #[test]
    fn test_speed_preset() {
        assert_eq!(SpeedPreset::Normal.to_multiplier(), 1.0);
        assert_eq!(SpeedPreset::Double.to_multiplier(), 2.0);
        assert_eq!(SpeedPreset::from_multiplier(5.0), SpeedPreset::Five);
        assert_eq!(SpeedPreset::from_multiplier(3.5), SpeedPreset::Custom(3.5));
    }

    #[test]
    fn test_replay_state() {
        let state = ReplayState::new();
        assert_eq!(state.status, PlaybackStatus::Stopped);
        assert!(!state.is_loaded);
        assert_eq!(state.progress(), 0.0);
    }

    #[test]
    fn test_replay_data() {
        let trades = vec![
            Trade {
                time: Timestamp::from_millis(1000),
                price: Price::from_f32(100.0),
                quantity: Quantity(10.0),
                side: Side::Buy,
            },
            Trade {
                time: Timestamp::from_millis(2000),
                price: Price::from_f32(101.0),
                quantity: Quantity(20.0),
                side: Side::Sell,
            },
        ];

        let ticker_info = FuturesTickerInfo {
            ticker: FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex),
            tick_size: 0.25,
            min_qty: 1.0,
            contract_size: 50.0,
        };

        let data = ReplayData::new(trades, vec![], ticker_info);

        assert_eq!(data.trades.len(), 2);
        assert_eq!(data.time_range.start.to_millis(), 1000);
        assert_eq!(data.time_range.end.to_millis(), 2000);

        let window_trades = data.trades_in_window(1000, 2000);
        assert_eq!(window_trades.len(), 2);
    }
}
