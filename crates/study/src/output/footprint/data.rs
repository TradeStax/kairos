//! Footprint data: top-level container, per-candle, and per-level structs.
//!
//! Produced by the footprint study and consumed by the chart renderer to
//! draw per-candle trade-level detail (boxes, profiles, or heatmaps).

use data::SerializableColor;

use super::render::*;
use super::scaling::*;

/// Top-level footprint data produced by the footprint study.
///
/// Contains all candle data along with the rendering parameters needed
/// to draw the footprint overlay.
#[derive(Debug, Clone)]
pub struct FootprintData {
    // -- Core --
    /// Rendering mode (Box or Profile).
    pub mode: FootprintRenderMode,
    /// Data type to display in each cell.
    pub data_type: FootprintDataType,
    /// Scaling strategy for bar widths.
    pub scaling: FootprintScaling,
    /// Position of the thin candle body marker.
    pub candle_position: FootprintCandlePosition,
    /// Per-candle footprint data.
    pub candles: Vec<FootprintCandle>,

    // -- Bar Marker --
    /// Width of the outside bar marker as a fraction of cell width.
    pub bar_marker_width: f32,
    /// Style for the thin candle drawn outside footprint bars.
    pub outside_bar_style: OutsideBarStyle,
    /// Whether to draw a border around the outside bar.
    pub show_outside_border: bool,
    /// Maximum number of footprint bars to render (LOD cutoff).
    pub max_bars_to_show: usize,

    // -- Background --
    /// How cell backgrounds are colored.
    pub bg_color_mode: BackgroundColorMode,
    /// Maximum alpha for background intensity coloring.
    pub bg_max_alpha: f32,
    /// Custom buy-side background color (overrides theme default).
    pub bg_buy_color: Option<SerializableColor>,
    /// Custom sell-side background color (overrides theme default).
    pub bg_sell_color: Option<SerializableColor>,
    /// Draw horizontal grid lines between price levels.
    pub show_grid_lines: bool,

    // -- Text --
    /// Base font size for level values in logical pixels.
    pub font_size: f32,
    /// How to format the numeric values in cells.
    pub text_format: TextFormat,
    /// Automatically scale text size based on cell dimensions.
    pub dynamic_text_size: bool,
    /// Whether to display cells with zero volume.
    pub show_zero_values: bool,
    /// Custom text color (overrides theme default).
    pub text_color: Option<SerializableColor>,

    // -- Tick grouping --
    /// How levels are grouped (manual or automatic).
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

/// Per-candle footprint data containing OHLC and trade levels.
#[derive(Debug, Clone)]
pub struct FootprintCandle {
    /// X coordinate: timestamp_ms (time-based) or candle index
    /// (tick-based).
    pub x: u64,
    /// Open price in fixed-point units (10^-8).
    pub open: i64,
    /// High price in fixed-point units (10^-8).
    pub high: i64,
    /// Low price in fixed-point units (10^-8).
    pub low: i64,
    /// Close price in fixed-point units (10^-8).
    pub close: i64,
    /// Trade volume at each price level within this candle.
    pub levels: Vec<FootprintLevel>,
    /// Index of the Point of Control level (highest total volume).
    pub poc_index: Option<usize>,
    /// Grouping quantum used for this candle (price units per row).
    pub quantum: i64,
}

/// Per-price-level trade data within a footprint candle.
#[derive(Debug, Clone, Copy)]
pub struct FootprintLevel {
    /// Price in fixed-point units (10^-8).
    pub price: i64,
    /// Buy-side (ask) volume at this level.
    pub buy_volume: f32,
    /// Sell-side (bid) volume at this level.
    pub sell_volume: f32,
}

impl FootprintLevel {
    /// Total volume (buy + sell) at this level.
    pub fn total_qty(&self) -> f32 {
        self.buy_volume + self.sell_volume
    }

    /// Net delta (buy - sell) at this level.
    pub fn delta_qty(&self) -> f32 {
        self.buy_volume - self.sell_volume
    }
}
