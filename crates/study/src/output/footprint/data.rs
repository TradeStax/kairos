//! Footprint data: top-level container, per-candle, and per-level structs.

use data::SerializableColor;

use super::render::*;
use super::scaling::*;

/// Top-level footprint data produced by
/// `FootprintStudy::output()`.
#[derive(Debug, Clone)]
pub struct FootprintData {
    // Core
    pub mode: FootprintRenderMode,
    pub data_type: FootprintDataType,
    pub scaling: FootprintScaling,
    pub candle_position: FootprintCandlePosition,
    pub candles: Vec<FootprintCandle>,
    // Bar Marker
    pub bar_marker_width: f32,
    pub outside_bar_style: OutsideBarStyle,
    pub show_outside_border: bool,
    pub max_bars_to_show: usize,
    // Background
    pub bg_color_mode: BackgroundColorMode,
    pub bg_max_alpha: f32,
    pub bg_buy_color: Option<SerializableColor>,
    pub bg_sell_color: Option<SerializableColor>,
    pub show_grid_lines: bool,
    // Text
    pub font_size: f32,
    pub text_format: TextFormat,
    pub dynamic_text_size: bool,
    pub show_zero_values: bool,
    pub text_color: Option<SerializableColor>,
    // Tick grouping
    pub grouping_mode: FootprintGroupingMode,
}

impl Default for FootprintData {
    fn default() -> Self {
        Self {
            mode: FootprintRenderMode::default(),
            data_type: FootprintDataType::default(),
            scaling: FootprintScaling::default(),
            candle_position: FootprintCandlePosition::default(),
            candles: Vec::new(),
            bar_marker_width: 0.25,
            outside_bar_style: OutsideBarStyle::default(),
            show_outside_border: false,
            max_bars_to_show: 200,
            bg_color_mode: BackgroundColorMode::default(),
            bg_max_alpha: 0.6,
            bg_buy_color: None,
            bg_sell_color: None,
            show_grid_lines: true,
            font_size: 11.0,
            text_format: TextFormat::default(),
            dynamic_text_size: true,
            show_zero_values: false,
            text_color: None,
            grouping_mode: FootprintGroupingMode::Manual,
        }
    }
}

/// Per-candle footprint data.
#[derive(Debug, Clone)]
pub struct FootprintCandle {
    /// X coordinate: timestamp_ms (time-based) or candle index
    /// (tick-based)
    pub x: u64,
    pub open: i64,
    pub high: i64,
    pub low: i64,
    pub close: i64,
    pub levels: Vec<FootprintLevel>,
    pub poc_index: Option<usize>,
    /// Grouping quantum used for this candle (price units per row)
    pub quantum: i64,
}

/// Per-price-level trade data within a footprint candle.
#[derive(Debug, Clone)]
pub struct FootprintLevel {
    pub price: i64,
    pub buy_volume: f32,
    pub sell_volume: f32,
}

impl FootprintLevel {
    pub fn total_qty(&self) -> f32 {
        self.buy_volume + self.sell_volume
    }

    pub fn delta_qty(&self) -> f32 {
        self.buy_volume - self.sell_volume
    }
}
