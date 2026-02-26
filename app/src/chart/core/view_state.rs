//! Chart View State
//!
//! Contains the visual state of a chart: position, zoom, price/time ranges, etc.

use super::Caches;
use crate::chart::scale::linear::PriceInfoLabel;
use crate::style::tokens;
use data::FuturesTickerInfo;
use data::{ChartBasis, ViewConfig};
use data::{Price, PriceStep};
use iced::{Length, Rectangle, Size, Vector};
use std::cell::Cell;

const TEXT_SIZE: f32 = tokens::text::BODY;

/// Convert an X canvas coordinate to an interval value (timestamp or tick index).
///
/// `offset` is the latest known interval value (timestamp ms or tick index) used
/// as the anchor for the conversion. `cell_width` is the pixel width per interval.
pub fn x_to_interval(x: f32, offset: f64, cell_width: f32, basis: &ChartBasis) -> u64 {
    match basis {
        ChartBasis::Time(timeframe) => {
            let interval = timeframe.to_milliseconds() as f64;
            let offset = offset as u64;
            if x <= 0.0 {
                let diff = (f64::from(-x / cell_width) * interval) as u64;
                offset.saturating_sub(diff)
            } else {
                let diff = (f64::from(x / cell_width) * interval) as u64;
                offset.saturating_add(diff)
            }
        }
        ChartBasis::Tick(_) => {
            let tick = -(x / cell_width);
            if tick < 0.0 { 0 } else { tick.round() as u64 }
        }
    }
}

/// Crosshair synchronization state for a chart.
///
/// Uses `Cell` for interior mutability so the main chart's `draw()` can
/// write and the study/side-panel `draw()` can read in the same frame.
pub struct CrosshairState {
    /// Current crosshair interval (snapped timestamp ms or tick index).
    /// Set whenever the cursor is in the main chart or study panel canvas.
    ///
    // INVARIANT: written in main chart draw(), read in panel/side_panel draw() — same frame.
    // Cell is used because Iced's canvas::Program::draw takes &self.
    // The draw call order (main → panel → side_panel) is enforced by Iced's layer stacking.
    pub interval: Cell<Option<u64>>,
    /// Current cursor Y in canvas-local screen coords for the side panel.
    ///
    // INVARIANT: written in main chart draw(), read in panel/side_panel draw() — same frame.
    // Cell is used because Iced's canvas::Program::draw takes &self.
    // The draw call order (main → panel → side_panel) is enforced by Iced's layer stacking.
    pub y: Cell<Option<f32>>,
    /// Remote crosshair interval from a linked pane.
    pub remote: Option<u64>,
}

impl CrosshairState {
    pub fn new() -> Self {
        CrosshairState {
            interval: Cell::new(None),
            y: Cell::new(None),
            remote: None,
        }
    }
}

impl Default for CrosshairState {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete view state for a chart
///
/// Contains all information needed to render the chart at its current
/// position and zoom level.
pub struct ViewState {
    /// Rendering caches
    pub cache: Caches,
    /// Canvas bounds in screen coordinates
    pub bounds: Rectangle,
    /// Current translation (pan offset)
    pub translation: Vector,
    /// Current scaling factor (zoom level)
    pub scaling: f32,
    /// Width of each data cell in pixels
    pub cell_width: f32,
    /// Height of each price level in pixels
    pub cell_height: f32,
    /// Time or tick basis for the chart
    pub basis: ChartBasis,
    /// Last traded price for marker
    pub last_price: Option<PriceInfoLabel>,
    /// Base price for Y-axis calculations
    pub base_price_y: Price,
    /// Latest X value (timestamp or tick index)
    pub latest_x: u64,
    /// Tick size for price rounding
    pub tick_size: PriceStep,
    /// Number of decimal places for price display
    pub decimals: usize,
    /// Ticker information
    pub ticker_info: FuturesTickerInfo,
    /// Layout configuration (splits, autoscale)
    pub layout: ViewConfig,
    /// Crosshair synchronization state (interior-mutable for same-frame write/read).
    pub crosshair: CrosshairState,
}

impl ViewState {
    /// Create new view state with initial configuration
    pub fn new(
        basis: ChartBasis,
        tick_size: PriceStep,
        decimals: usize,
        ticker_info: FuturesTickerInfo,
        layout: ViewConfig,
        cell_width: f32,
        cell_height: f32,
    ) -> Self {
        ViewState {
            cache: Caches::default(),
            bounds: Rectangle::default(),
            translation: Vector::default(),
            scaling: 1.0,
            cell_width,
            cell_height,
            basis,
            last_price: None,
            base_price_y: Price::from_f32(0.0),
            latest_x: 0,
            tick_size,
            decimals,
            ticker_info,
            layout,
            crosshair: CrosshairState::new(),
        }
    }

    /// Get the price unit scale factor
    #[inline]
    pub fn price_unit() -> i64 {
        10i64.pow(Price::PRICE_SCALE as u32)
    }

    /// Calculate the visible region in chart coordinates
    pub fn visible_region(&self, size: Size) -> Rectangle {
        let width = size.width / self.scaling;
        let height = size.height / self.scaling;

        Rectangle {
            x: -self.translation.x - width / 2.0,
            y: -self.translation.y - height / 2.0,
            width,
            height,
        }
    }

    /// Check if an interval X coordinate is within the visible region
    pub fn is_interval_x_visible(&self, interval_x: f32) -> bool {
        let region = self.visible_region(self.bounds.size());
        interval_x >= region.x && interval_x <= region.x + region.width
    }

    /// Get the visible interval range (start, end)
    pub fn interval_range(&self, region: &Rectangle) -> (u64, u64) {
        match self.basis {
            ChartBasis::Tick(_) => (
                self.x_to_interval(region.x + region.width),
                self.x_to_interval(region.x),
            ),
            ChartBasis::Time(timeframe) => {
                let interval = timeframe.to_milliseconds();
                (
                    self.x_to_interval(region.x).saturating_sub(interval / 2),
                    self.x_to_interval(region.x + region.width)
                        .saturating_add(interval / 2),
                )
            }
        }
    }

    /// Get the visible price range (highest, lowest)
    pub fn price_range(&self, region: &Rectangle) -> (Price, Price) {
        let highest = self.y_to_price(region.y);
        let lowest = self.y_to_price(region.y + region.height);
        (highest, lowest)
    }

    /// Convert interval value to X coordinate
    pub fn interval_to_x(&self, value: u64) -> f32 {
        match self.basis {
            ChartBasis::Time(timeframe) => {
                let interval = timeframe.to_milliseconds() as f64;
                let cell_width = f64::from(self.cell_width);

                let diff = value as f64 - self.latest_x as f64;
                (diff / interval * cell_width) as f32
            }
            ChartBasis::Tick(_) => -((value as f32) * self.cell_width),
        }
    }

    /// Convert X coordinate to interval value
    pub fn x_to_interval(&self, x: f32) -> u64 {
        x_to_interval(x, self.latest_x as f64, self.cell_width, &self.basis)
    }

    /// Convert price to Y coordinate
    pub fn price_to_y(&self, price: Price) -> f32 {
        if self.tick_size.units == 0 {
            let one = Self::price_unit() as f32;
            let delta_units = (self.base_price_y.units() - price.units()) as f32;
            return (delta_units / one) * self.cell_height;
        }

        let delta_units = self.base_price_y.units() - price.units();
        let ticks = (delta_units as f32) / (self.tick_size.units as f32);
        ticks * self.cell_height
    }

    /// Convert Y coordinate to price
    pub fn y_to_price(&self, y: f32) -> Price {
        if self.tick_size.units == 0 {
            let one = Self::price_unit() as f32;
            let delta_units = ((y / self.cell_height) * one).round() as i64;
            return Price::from_units(self.base_price_y.units() - delta_units);
        }

        let ticks: f32 = y / self.cell_height;
        let delta_units = (ticks * self.tick_size.units as f32).round() as i64;
        Price::from_units(self.base_price_y.units() - delta_units)
    }

    /// Get the current layout configuration
    pub fn layout(&self) -> ViewConfig {
        ViewConfig {
            splits: self.layout.splits.clone(),
            autoscale: self.layout.autoscale,
            side_splits: self.layout.side_splits.clone(),
        }
    }

    /// Calculate Y-axis label width based on tick size
    pub fn y_labels_width(&self) -> Length {
        let tick_size = self.ticker_info.min_ticksize();

        // Calculate decimal places from tick size
        let tick_f32 = tick_size.to_f32();
        let decimal_places = if tick_f32 >= 1.0 {
            0
        } else if tick_f32 > 0.0 {
            (-tick_f32.log10()).ceil() as usize
        } else {
            0 // Safe default before ticker is loaded
        };

        let value = format!(
            "{:.prec$}",
            self.base_price_y.to_f32(),
            prec = decimal_places
        );
        let width = (value.len() as f32 * TEXT_SIZE * 0.8).max(72.0);

        Length::Fixed(width.ceil())
    }

    /// Snap X position to nearest interval and return (interval_value, snap_ratio)
    pub fn snap_x_to_index(&self, x: f32, bounds: Size, region: Rectangle) -> (u64, f32) {
        let x_ratio = x / bounds.width;

        match self.basis {
            ChartBasis::Time(timeframe) => {
                let interval = timeframe.to_milliseconds();
                let earliest = self.x_to_interval(region.x) as f64;
                let latest = self.x_to_interval(region.x + region.width) as f64;

                let millis_at_x = earliest + f64::from(x_ratio) * (latest - earliest);

                let rounded_timestamp = (millis_at_x / (interval as f64)).round() as u64 * interval;

                let snap_ratio = if latest - earliest > 0.0 {
                    ((rounded_timestamp as f64 - earliest) / (latest - earliest)) as f32
                } else {
                    0.5
                };

                (rounded_timestamp, snap_ratio)
            }
            ChartBasis::Tick(aggregation) => {
                let (chart_x_min, chart_x_max) = (region.x, region.x + region.width);
                let chart_x = chart_x_min + x_ratio * (chart_x_max - chart_x_min);

                let cell_index = (chart_x / self.cell_width).round();
                let snapped_x = cell_index * self.cell_width;

                let snap_ratio = if chart_x_max - chart_x_min > 0.0 {
                    (snapped_x - chart_x_min) / (chart_x_max - chart_x_min)
                } else {
                    0.5
                };

                let rounded_tick = (-cell_index as u64) * u64::from(aggregation);

                (rounded_tick, snap_ratio)
            }
        }
    }
}
