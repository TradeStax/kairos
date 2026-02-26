//! Comparison chart for multi-ticker analysis
//!
//! This module provides a chart that displays multiple futures contracts
//! on a single synchronized chart with optional normalization.

mod legend;
mod line_widget;
mod render;
mod scene;
mod series;
pub(crate) mod types;

pub use series::TickerSeriesEditor;

use line_widget::{DEFAULT_ZOOM_POINTS, LineComparison, LineComparisonEvent};
use types::{Series, Zoom};

use crate::screen::dashboard::pane::config::ComparisonConfig;
use data::FuturesTickerInfo;
use data::{ChartBasis, ChartData, Timeframe};

use rustc_hash::FxHashMap;
use std::time::Instant;

const DEFAULT_PAN_POINTS: f32 = 8.0;

/// Shared text size for all comparison chart sub-modules (legend, render, scene).
pub(super) const TEXT_SIZE: f32 = crate::style::tokens::text::BODY;

/// Comparison chart actions that can be taken by parent
pub enum Action {
    SeriesColorChanged(FuturesTickerInfo, iced::Color),
    SeriesNameChanged(FuturesTickerInfo, String),
    RemoveSeries(FuturesTickerInfo),
    OpenSeriesEditor,
}

/// Multi-ticker comparison chart
///
/// Displays multiple futures contracts on a single synchronized chart with optional normalization.
/// All data is pre-loaded from ChartData - no real-time updates or historical fetching.
pub struct ComparisonChart {
    /// Rendered series for each ticker
    series: Vec<Series>,

    /// Index mapping ticker to series position
    series_index: FxHashMap<FuturesTickerInfo, usize>,

    /// Timeframe (for time-based charts)
    timeframe: Timeframe,

    /// Whether prices are normalized to 100
    normalized: bool,

    /// Original non-normalized data for toggle support
    original_data: Vec<(FuturesTickerInfo, Vec<(u64, f32)>)>,

    /// Zoom level
    zoom: Zoom,

    /// Pan offset
    pan: f32,

    /// User configuration (colors, names, normalization)
    config: ComparisonConfig,

    /// Series editor state
    series_editor: TickerSeriesEditor,

    /// Cache revision for widget invalidation
    cache_rev: u64,

    /// Last update timestamp
    last_tick: Instant,
}

#[derive(Debug, Clone)]
pub enum Message {
    Chart(LineComparisonEvent),
    Editor(series::Message),
    OpenEditorFor(FuturesTickerInfo),
}

impl ComparisonChart {
    /// Create a new comparison chart from pre-loaded chart data
    ///
    /// # Arguments
    /// * `tickers_data` - Vector of (ticker_info, chart_data) tuples
    /// * `basis` - Chart basis (time or tick)
    /// * `config` - Optional user configuration (colors, names, normalization)
    ///
    /// # Returns
    /// A fully initialized comparison chart ready to render
    pub fn from_multi_chart_data(
        tickers_data: Vec<(FuturesTickerInfo, ChartData)>,
        basis: ChartBasis,
        config: Option<ComparisonConfig>,
    ) -> Self {
        // Handle both time and tick basis
        let timeframe = match basis {
            ChartBasis::Time(tf) => tf,
            ChartBasis::Tick(tick_count) => {
                // For tick basis, estimate equivalent timeframe for display
                // This is approximate - tick charts use candle indices, not time
                log::info!(
                    "ComparisonChart: Using tick basis ({}T) - timestamps from candle data",
                    tick_count
                );
                // Use M1 as default timeframe for axis labeling
                Timeframe::M1
            }
        };

        let cfg = config.unwrap_or_default();

        let color_map: FxHashMap<String, iced::Color> = cfg
            .colors
            .iter()
            .map(|(s, r)| (s.clone(), crate::style::theme::rgba_to_iced_color(*r)))
            .collect();
        let name_map: FxHashMap<String, String> = cfg.names.iter().cloned().collect();

        let mut series = Vec::with_capacity(tickers_data.len());
        let mut series_index = FxHashMap::default();
        let mut original_data = Vec::with_capacity(tickers_data.len());

        for (i, (ticker_info, chart_data)) in tickers_data.iter().enumerate() {
            let ticker_str = ticker_info.ticker.as_str().to_string();

            let color = color_map
                .get(&ticker_str)
                .copied()
                .unwrap_or_else(|| default_color_for(ticker_info));
            let name = name_map.get(&ticker_str).cloned();

            // Convert ChartData candles to (timestamp, close_price) points
            // For tick basis, use actual candle timestamps (not indices)
            let points: Vec<(u64, f32)> = chart_data
                .candles
                .iter()
                .map(|candle| (candle.time.to_millis(), candle.close.to_f32()))
                .collect();

            // Store original data for denormalization support
            original_data.push((*ticker_info, points.clone()));

            let ser = Series {
                ticker_info: ticker_info_to_old_format(*ticker_info),
                name,
                points,
                color,
            };

            series.push(ser);
            series_index.insert(*ticker_info, i);
        }

        // Normalize prices if configured
        let normalized = cfg.normalize.unwrap_or(false);
        if normalized {
            normalize_series(&mut series);
        }

        Self {
            series,
            series_index,
            timeframe,
            normalized,
            original_data,
            zoom: Zoom(DEFAULT_ZOOM_POINTS),
            pan: DEFAULT_PAN_POINTS,
            config: cfg,
            series_editor: TickerSeriesEditor::default(),
            cache_rev: 0,
            last_tick: Instant::now(),
        }
    }

    /// Update the chart with a message
    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::Chart(event) => match event {
                LineComparisonEvent::ZoomChanged(zoom) => {
                    self.zoom = zoom;
                    None
                }
                LineComparisonEvent::PanChanged(pan) => {
                    self.pan = pan;
                    None
                }
                LineComparisonEvent::SeriesCog(ticker_info) => {
                    let futures_ticker_info = old_format_to_ticker_info(&ticker_info);
                    self.open_editor_for_ticker(futures_ticker_info)
                }
                LineComparisonEvent::SeriesRemove(ticker_info) => {
                    let futures_ticker_info = old_format_to_ticker_info(&ticker_info);
                    Some(Action::RemoveSeries(futures_ticker_info))
                }
                LineComparisonEvent::XAxisDoubleClick => {
                    self.zoom = Zoom(DEFAULT_ZOOM_POINTS);
                    self.pan = DEFAULT_PAN_POINTS;
                    None
                }
            },
            Message::Editor(msg) => self.series_editor.update(msg),
            Message::OpenEditorFor(ticker_info) => self.open_editor_for_ticker(ticker_info),
        }
    }

    /// Render the chart
    pub fn view(&self, timezone: crate::config::UserTimezone) -> iced::Element<'_, Message> {
        if self.series.iter().all(|s| s.points.is_empty()) {
            return iced::widget::center(iced::widget::text("Waiting for data...").size(16)).into();
        }

        let chart: iced::Element<_> = LineComparison::<Series>::new(&self.series, self.timeframe)
            .with_timezone(timezone)
            .with_zoom(self.zoom)
            .with_pan(self.pan)
            .version(self.cache_rev)
            .into();

        iced::widget::container(chart.map(Message::Chart))
            .padding(1)
            .into()
    }

    /// Get last update timestamp
    pub fn last_update(&self) -> Instant {
        self.last_tick
    }

    /// Get reference to the series
    pub fn series(&self) -> &[Series] {
        &self.series
    }

    /// Get reference to the series editor
    pub fn series_editor(&self) -> &TickerSeriesEditor {
        &self.series_editor
    }

    /// Add a new ticker to the comparison
    pub fn add_ticker(
        &mut self,
        ticker_info: &FuturesTickerInfo,
        chart_data: ChartData,
    ) -> Result<(), String> {
        if self.series_index.contains_key(ticker_info) {
            return Err("Ticker already exists".to_string());
        }

        let ticker_str = ticker_info.ticker.as_str().to_string();
        let color = self
            .config
            .colors
            .iter()
            .find(|(t, _)| t == &ticker_str)
            .map(|(_, r)| crate::style::theme::rgba_to_iced_color(*r))
            .unwrap_or_else(|| default_color_for(ticker_info));
        let name = self
            .config
            .names
            .iter()
            .find(|(t, _)| t == &ticker_str)
            .map(|(_, n)| n.clone());

        // Convert ChartData candles to points
        // Works for both time and tick basis - uses actual candle timestamps
        let points: Vec<(u64, f32)> = chart_data
            .candles
            .iter()
            .map(|candle| (candle.time.to_millis(), candle.close.to_f32()))
            .collect();

        // Store original data
        self.original_data.push((*ticker_info, points.clone()));

        let new_series = Series {
            ticker_info: ticker_info_to_old_format(*ticker_info),
            name,
            points,
            color,
        };

        let idx = self.series.len();
        self.series.push(new_series);
        self.series_index.insert(*ticker_info, idx);

        // Re-normalize all series if needed
        if self.normalized {
            normalize_series(&mut self.series);
        }

        // Rebuild index
        self.rebuild_series_index();
        self.cache_rev = self.cache_rev.wrapping_add(1);
        Ok(())
    }

    /// Remove a ticker from the comparison
    pub fn remove_ticker(&mut self, ticker_info: &FuturesTickerInfo) {
        if let Some(idx) = self.series_index.remove(ticker_info) {
            self.series.remove(idx);

            // Remove from original data
            if let Some(pos) = self
                .original_data
                .iter()
                .position(|(t, _)| t == ticker_info)
            {
                self.original_data.remove(pos);
            }

            // Rebuild index
            self.rebuild_series_index();
        }

        // Close editor if open for this ticker
        if self
            .series_editor
            .show_config_for
            .is_some_and(|t| t == *ticker_info)
        {
            self.series_editor.show_config_for = None;
        }

        self.cache_rev = self.cache_rev.wrapping_add(1);
    }

    /// Set series color
    pub fn set_series_color(&mut self, ticker: FuturesTickerInfo, color: iced::Color) {
        if let Some(idx) = self.series_index.get(&ticker)
            && let Some(s) = self.series.get_mut(*idx)
        {
            s.color = color;
            self.upsert_config_color(ticker, color);
            self.cache_rev = self.cache_rev.wrapping_add(1)
        }
    }

    /// Set series name
    pub fn set_series_name(&mut self, ticker: FuturesTickerInfo, name: String) {
        let clamped = Self::clamp_label(name.trim());
        if let Some(idx) = self.series_index.get(&ticker)
            && let Some(s) = self.series.get_mut(*idx)
        {
            s.name = if clamped.is_empty() {
                None
            } else {
                Some(clamped)
            };

            self.cache_rev = self.cache_rev.wrapping_add(1)
        }
    }

    /// Toggle normalization on/off
    pub fn set_normalized(&mut self, normalized: bool) {
        if self.normalized == normalized {
            return;
        }

        self.normalized = normalized;

        if normalized {
            // Normalize from current state
            normalize_series(&mut self.series);
        } else {
            // Restore original data
            for (ticker_info, original_points) in &self.original_data {
                if let Some(idx) = self.series_index.get(ticker_info)
                    && let Some(series) = self.series.get_mut(*idx)
                {
                    series.points = original_points.clone();
                }
            }
        }

        self.cache_rev = self.cache_rev.wrapping_add(1);
    }

    /// Get serializable config for persistence
    pub fn serializable_config(&self) -> ComparisonConfig {
        let mut colors = vec![];
        let mut names = vec![];

        for s in &self.series {
            let futures_info = old_format_to_ticker_info(&s.ticker_info);
            let ticker_str = futures_info.ticker.as_str().to_string();

            colors.push((
                ticker_str.clone(),
                crate::style::theme::iced_color_to_rgba(s.color),
            ));
            if let Some(name) = &s.name {
                names.push((ticker_str, name.clone()));
            }
        }

        ComparisonConfig {
            colors,
            names,
            normalize: Some(self.normalized),
        }
    }

    /// Get list of selected tickers
    pub fn selected_tickers(&self) -> Vec<FuturesTickerInfo> {
        self.series
            .iter()
            .map(|s| old_format_to_ticker_info(&s.ticker_info))
            .collect()
    }

    // ── Private methods ────────────────────────────────────────────────

    fn open_editor_for_ticker(&mut self, ticker_info: FuturesTickerInfo) -> Option<Action> {
        self.series_editor.show_config_for = Some(ticker_info);

        if let Some(idx) = self.series_index.get(&ticker_info) {
            self.series_editor.editing_color = Some(crate::config::theme::rgba_to_hsva(
                crate::style::theme::iced_color_to_rgba(self.series[*idx].color),
            ));
            self.series_editor.editing_name = self.series[*idx].name.clone();
        } else {
            self.series_editor.editing_color = None;
            self.series_editor.editing_name = None;
        }

        Some(Action::OpenSeriesEditor)
    }

    fn rebuild_series_index(&mut self) {
        self.series_index.clear();
        for (i, s) in self.series.iter().enumerate() {
            let futures_info = old_format_to_ticker_info(&s.ticker_info);
            self.series_index.insert(futures_info, i);
        }
    }

    fn upsert_config_color(&mut self, ticker_info: FuturesTickerInfo, color: iced::Color) {
        let ticker_str = ticker_info.ticker.as_str().to_string();
        let rgba = crate::style::theme::iced_color_to_rgba(color);
        if let Some((_, c)) = self
            .config
            .colors
            .iter_mut()
            .find(|(t, _)| t == &ticker_str)
        {
            *c = rgba;
        } else {
            self.config.colors.push((ticker_str, rgba));
        }
    }

    fn clamp_label(name: &str) -> String {
        name.chars().take(24).collect()
    }
}

// ── Normalization ─────────────────────────────────────────────────────

/// Normalize all series to start at 100
///
/// This enables relative comparison by setting the first price of each series to 100
/// and scaling all subsequent prices proportionally.
///
/// ## Optimizations (Phase 6.2)
/// - Pre-computed scaling factors (single division)
/// - Vectorized operations where possible
/// - Early validation to skip empty series
fn normalize_series(series: &mut [Series]) {
    for s in series.iter_mut() {
        // Early exit for empty series
        if s.points.is_empty() {
            continue;
        }

        // Get first close price for normalization base
        let first_close = s.points[0].1;

        // Validate non-zero, non-NaN
        if first_close <= 0.0 || !first_close.is_finite() {
            log::warn!(
                "Cannot normalize series with invalid first price: {}",
                first_close
            );
            continue;
        }

        // Pre-compute scaling factor (single division, not per-point)
        let scale_factor = 100.0 / first_close;

        // Apply normalization with pre-computed factor
        for point in s.points.iter_mut() {
            point.1 *= scale_factor;
        }
    }
}

// ── Color Generation ──────────────────────────────────────────────────

/// Generate a default color for a ticker using deterministic hashing
fn default_color_for(ticker: &FuturesTickerInfo) -> iced::Color {
    use std::hash::{DefaultHasher, Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    ticker.ticker.as_str().hash(&mut hasher);
    let seed = hasher.finish();

    // Golden-angle distribution for hue (in degrees)
    let golden = 0.618_034_f32;
    let base = ((seed as f32 / u64::MAX as f32) + 0.12345).fract();
    let hue = (base + golden).fract() * 360.0;

    // Slightly vary saturation and value in a pleasant range
    let s = 0.60 + (((seed >> 8) & 0xFF) as f32 / 255.0) * 0.25; // 0.60..=0.85
    let v = 0.85 + (((seed >> 16) & 0x7F) as f32 / 127.0) * 0.10; // 0.85..=0.95

    crate::style::theme::rgba_to_iced_color(crate::config::theme::from_hsv_degrees_rgba(
        hue,
        s.min(1.0),
        v.min(1.0),
    ))
}

// ── Compatibility Layer (Temporary Bridge) ────────────────────────────
// These are now identity functions since Series.ticker_info is FuturesTickerInfo.

pub(crate) fn ticker_info_to_old_format(info: FuturesTickerInfo) -> FuturesTickerInfo {
    info
}

pub(crate) fn old_format_to_ticker_info(info: &FuturesTickerInfo) -> FuturesTickerInfo {
    *info
}
