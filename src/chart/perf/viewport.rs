//! Viewport Culling and Optimization
//!
//! Provides efficient viewport-based data filtering to avoid processing
//! off-screen elements.
//!
//! ## Key Optimizations
//! - Early rejection of off-screen data
//! - Binary search for time-range queries
//! - Spatial bounds checking
//! - Pre-filtered iterators

use std::ops::RangeInclusive;

/// Viewport bounds in chart coordinates
#[derive(Debug, Clone, Copy)]
pub struct ViewportBounds {
    /// Earliest visible time (milliseconds)
    pub time_start: u64,
    /// Latest visible time (milliseconds)
    pub time_end: u64,
    /// Highest visible price (in price units)
    pub price_high: i64,
    /// Lowest visible price (in price units)
    pub price_low: i64,
}

impl ViewportBounds {
    /// Create viewport bounds from time and price ranges
    pub fn new(time_range: (u64, u64), price_range: (i64, i64)) -> Self {
        Self {
            time_start: time_range.0,
            time_end: time_range.1,
            price_high: price_range.1,
            price_low: price_range.0,
        }
    }

    /// Check if a time is within visible range
    #[inline]
    pub fn contains_time(&self, time: u64) -> bool {
        time >= self.time_start && time <= self.time_end
    }

    /// Check if a price is within visible range
    #[inline]
    pub fn contains_price(&self, price: i64) -> bool {
        price >= self.price_low && price <= self.price_high
    }

    /// Check if a time-price point is visible
    #[inline]
    pub fn contains_point(&self, time: u64, price: i64) -> bool {
        self.contains_time(time) && self.contains_price(price)
    }

    /// Get time range as inclusive range for BTreeMap queries
    pub fn time_range(&self) -> RangeInclusive<u64> {
        self.time_start..=self.time_end
    }

    /// Get price range as inclusive range for BTreeMap queries
    pub fn price_range(&self) -> RangeInclusive<i64> {
        self.price_low..=self.price_high
    }

    /// Expand viewport bounds by a margin (for lookahead/smooth scrolling)
    pub fn with_margin(&self, time_margin_ms: u64, price_margin_units: i64) -> Self {
        Self {
            time_start: self.time_start.saturating_sub(time_margin_ms),
            time_end: self.time_end.saturating_add(time_margin_ms),
            price_high: self.price_high.saturating_add(price_margin_units),
            price_low: self.price_low.saturating_sub(price_margin_units),
        }
    }
}

/// Viewport culling strategies for different data structures
pub struct ViewportCuller;

impl ViewportCuller {
    /// Filter candles to only those in viewport
    ///
    /// Uses efficient range filtering for sorted candle data
    pub fn filter_candles<'a, C, F>(
        candles: &'a [C],
        time_range: RangeInclusive<u64>,
        time_accessor: F,
    ) -> impl Iterator<Item = (usize, &'a C)> + 'a
    where
        C: 'a,
        F: Fn(&C) -> u64 + 'a,
    {
        candles
            .iter()
            .enumerate()
            .filter(move |(_, c)| time_range.contains(&time_accessor(c)))
    }

    /// Binary search to find start index for time range
    ///
    /// Returns the index of the first item >= start_time
    pub fn binary_search_time_start<T, F>(
        items: &[T],
        start_time: u64,
        time_accessor: F,
    ) -> usize
    where
        F: Fn(&T) -> u64,
    {
        items
            .binary_search_by_key(&start_time, time_accessor)
            .unwrap_or_else(|i| i)
    }

    /// Binary search to find end index for time range
    ///
    /// Returns the index of the last item <= end_time + 1
    pub fn binary_search_time_end<T, F>(
        items: &[T],
        end_time: u64,
        time_accessor: F,
    ) -> usize
    where
        F: Fn(&T) -> u64,
    {
        items
            .binary_search_by_key(&(end_time + 1), time_accessor)
            .unwrap_or_else(|i| i)
    }

    /// Get slice of items within time range using binary search
    ///
    /// Much more efficient than filtering the entire slice
    pub fn slice_time_range<T, F>(
        items: &[T],
        time_range: RangeInclusive<u64>,
        time_accessor: F,
    ) -> &[T]
    where
        F: Fn(&T) -> u64 + Copy,
    {
        let start_idx = Self::binary_search_time_start(items, *time_range.start(), time_accessor);
        let end_idx = Self::binary_search_time_end(items, *time_range.end(), time_accessor);

        &items[start_idx..end_idx.min(items.len())]
    }
}

/// Spatial index for fast point lookups (future enhancement)
///
/// Simple grid-based spatial index for O(1) viewport queries
pub struct SpatialGrid<T> {
    /// Grid cells (time_bucket, price_bucket) -> items
    cells: std::collections::HashMap<(usize, usize), Vec<T>>,
    /// Time bucket size in milliseconds
    time_bucket_ms: u64,
    /// Price bucket size in units
    price_bucket_units: i64,
}

impl<T> SpatialGrid<T> {
    /// Create new spatial grid with bucket sizes
    pub fn new(time_bucket_ms: u64, price_bucket_units: i64) -> Self {
        Self {
            cells: std::collections::HashMap::new(),
            time_bucket_ms,
            price_bucket_units,
        }
    }

    /// Insert item at time-price location
    pub fn insert(&mut self, time: u64, price: i64, item: T) {
        let cell = self.get_cell(time, price);
        self.cells.entry(cell).or_default().push(item);
    }

    /// Query items in viewport
    pub fn query(&self, bounds: &ViewportBounds) -> impl Iterator<Item = &T> {
        let start_cell = self.get_cell(bounds.time_start, bounds.price_low);
        let end_cell = self.get_cell(bounds.time_end, bounds.price_high);

        let mut items = Vec::new();

        for time_bucket in start_cell.0..=end_cell.0 {
            for price_bucket in start_cell.1..=end_cell.1 {
                if let Some(cell_items) = self.cells.get(&(time_bucket, price_bucket)) {
                    items.extend(cell_items.iter());
                }
            }
        }

        items.into_iter()
    }

    /// Get grid cell for time-price coordinate
    fn get_cell(&self, time: u64, price: i64) -> (usize, usize) {
        let time_bucket = (time / self.time_bucket_ms) as usize;
        let price_bucket = if price >= 0 {
            (price / self.price_bucket_units) as usize
        } else {
            0
        };
        (time_bucket, price_bucket)
    }

    /// Clear the grid
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    /// Get memory usage estimate
    pub fn memory_usage(&self) -> usize {
        self.cells.len() * std::mem::size_of::<Vec<T>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_bounds() {
        let bounds = ViewportBounds::new((1000, 2000), (100, 200));

        assert!(bounds.contains_time(1500));
        assert!(!bounds.contains_time(500));

        assert!(bounds.contains_price(150));
        assert!(!bounds.contains_price(250));

        assert!(bounds.contains_point(1500, 150));
        assert!(!bounds.contains_point(1500, 250));
    }

    #[test]
    fn test_viewport_margin() {
        let bounds = ViewportBounds::new((1000, 2000), (100, 200));
        let expanded = bounds.with_margin(500, 50);

        assert_eq!(expanded.time_start, 500);
        assert_eq!(expanded.time_end, 2500);
        assert_eq!(expanded.price_low, 50);
        assert_eq!(expanded.price_high, 250);
    }

    #[test]
    fn test_viewport_culler_binary_search() {
        #[derive(Debug)]
        struct Item {
            time: u64,
        }

        let items = vec![
            Item { time: 1000 },
            Item { time: 2000 },
            Item { time: 3000 },
            Item { time: 4000 },
            Item { time: 5000 },
        ];

        let start = ViewportCuller::binary_search_time_start(&items, 2500, |i| i.time);
        let end = ViewportCuller::binary_search_time_end(&items, 4500, |i| i.time);

        assert_eq!(start, 2); // First item >= 2500 is at index 2 (time=3000)
        // binary_search_time_end searches for 4501; insertion point is index 4
        // (between time=4000 at [3] and time=5000 at [4])
        assert_eq!(end, 4);

        let slice = ViewportCuller::slice_time_range(&items, 2500..=4500, |i| i.time);
        assert_eq!(slice.len(), 2); // Items at time=3000, time=4000 (5000 > 4500)
    }

    #[test]
    fn test_spatial_grid() {
        let mut grid = SpatialGrid::new(1000, 10);

        grid.insert(1500, 105, "trade1");
        // Place trade2 far enough outside the viewport that it falls in a
        // different grid cell than any cell covered by the query bounds.
        // Viewport end time=2000 -> bucket 2, end price=200 -> bucket 20.
        // Use time bucket 3 (time >= 3000) to be outside the query range.
        grid.insert(3500, 305, "trade2");
        grid.insert(1600, 110, "trade3");

        let bounds = ViewportBounds::new((1000, 2000), (100, 200));
        let results: Vec<_> = grid.query(&bounds).collect();

        // Spatial grid uses coarse bucket-level filtering, so items near
        // bucket boundaries may be included even if outside exact bounds.
        // trade2 (2500, 205) falls in bucket (2, 20) which overlaps the
        // query range of buckets (1..=2, 10..=20), so all 3 items are returned.
        assert_eq!(results.len(), 3);
    }
}
