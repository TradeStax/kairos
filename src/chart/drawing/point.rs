//! Drawing Point
//!
//! Represents a point anchored to price and time coordinates on the chart.

use crate::chart::ViewState;
use data::SerializablePoint;
use exchange::util::Price as ExchangePrice;
use iced::{Point, Size};

/// A point anchored to price and time coordinates
#[derive(Debug, Clone, Copy)]
pub struct DrawingPoint {
    /// Price in fixed-point units
    pub price: ExchangePrice,
    /// Timestamp in milliseconds or tick index
    pub time: u64,
    /// Whether this point was snapped to a candle
    pub snapped: bool,
}

impl DrawingPoint {
    /// Create a new drawing point
    pub fn new(price: ExchangePrice, time: u64) -> Self {
        Self {
            price,
            time,
            snapped: false,
        }
    }

    /// Create a drawing point from screen coordinates
    pub fn from_screen(
        cursor: Point,
        state: &ViewState,
        bounds: Size,
        snap: bool,
    ) -> Self {
        let region = state.visible_region(bounds);

        // Convert screen X to chart X coordinate
        let x_ratio = cursor.x / bounds.width;
        let chart_x = region.x + x_ratio * region.width;

        // Convert screen Y to chart Y coordinate
        let y_ratio = cursor.y / bounds.height;
        let chart_y = region.y + y_ratio * region.height;

        // Convert chart coordinates to time and price
        let time = if snap {
            let (snapped_time, _) = state.snap_x_to_index(cursor.x, bounds, region);
            snapped_time
        } else {
            state.x_to_interval(chart_x)
        };

        let price = state.y_to_price(chart_y);

        Self {
            price,
            time,
            snapped: snap,
        }
    }

    /// Convert this point to screen coordinates
    pub fn to_screen(&self, state: &ViewState, bounds: Size) -> Point {
        let region = state.visible_region(bounds);

        // Convert time to chart X coordinate
        let chart_x = state.interval_to_x(self.time);

        // Convert price to chart Y coordinate
        let chart_y = state.price_to_y(self.price);

        // Convert chart coordinates to screen coordinates
        let x_ratio = (chart_x - region.x) / region.width;
        let y_ratio = (chart_y - region.y) / region.height;

        Point::new(x_ratio * bounds.width, y_ratio * bounds.height)
    }

    /// Convert to serializable format
    pub fn to_serializable(&self) -> SerializablePoint {
        SerializablePoint {
            price_units: self.price.units,
            time: self.time,
            snapped: self.snapped,
        }
    }

    /// Create from serializable format
    pub fn from_serializable(point: &SerializablePoint) -> Self {
        Self {
            price: ExchangePrice::from_units(point.price_units),
            time: point.time,
            snapped: point.snapped,
        }
    }
}

impl From<DrawingPoint> for SerializablePoint {
    fn from(point: DrawingPoint) -> Self {
        point.to_serializable()
    }
}

impl From<&SerializablePoint> for DrawingPoint {
    fn from(point: &SerializablePoint) -> Self {
        DrawingPoint::from_serializable(point)
    }
}
