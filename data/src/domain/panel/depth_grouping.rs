//! Depth Grouping - Pure Domain Logic
//!
//! Groups orderbook depth levels by tick size.
//! Used for ladder display with configurable tick grouping.
//!
//! All prices are in exchange::util::Price units (i64) for precision.

use std::collections::BTreeMap;

/// Order book side for depth grouping (Bid/Ask).
///
/// Distinct from `crate::domain::types::Side` (Buy/Sell) which represents
/// trade aggressor side. This enum is specifically for orderbook-level
/// operations where rounding direction matters (bids floor, asks ceil).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthSide {
    Bid,
    Ask,
}

impl DepthSide {
    pub fn is_bid(self) -> bool {
        matches!(self, DepthSide::Bid)
    }

    pub fn is_ask(self) -> bool {
        matches!(self, DepthSide::Ask)
    }
}

/// Group orderbook levels by tick size
///
/// This function rounds prices to tick multiples and aggregates quantities.
/// Uses side-biased rounding: bids round down (floor), asks round up (ceil).
///
/// # Arguments
/// * `levels` - Raw orderbook levels (price_units -> quantity)
/// * `side` - Bid or Ask side (determines rounding direction)
/// * `tick_step_units` - Tick size in price units (from exchange::util::PriceStep)
///
/// # Returns
/// Grouped levels (rounded_price_units -> aggregated_quantity)
///
/// # Example
/// ```ignore
/// use std::collections::BTreeMap;
/// use kairos_data::domain::panel::depth_grouping::{group_depth_by_tick, DepthSide};
///
/// let mut levels = BTreeMap::new();
/// levels.insert(100_000_000_i64, 10.0_f32);  // 1.00 @ 10
/// levels.insert(100_500_000_i64, 5.0_f32);   // 1.005 @ 5
/// levels.insert(101_000_000_i64, 8.0_f32);   // 1.01 @ 8
///
/// // Group to 0.01 tick (1_000_000 units at PRICE_SCALE=8)
/// let grouped = group_depth_by_tick(&levels, DepthSide::Bid, 1_000_000);
///
/// assert_eq!(grouped.len(), 2);
/// assert_eq!(grouped.get(&100_000_000), Some(&15.0));  // 10 + 5 grouped
/// assert_eq!(grouped.get(&101_000_000), Some(&8.0));
/// ```
pub fn group_depth_by_tick(
    levels: &BTreeMap<i64, f32>,
    side: DepthSide,
    tick_step_units: i64,
) -> BTreeMap<i64, f32> {
    let mut grouped = BTreeMap::new();

    for (price_units, qty) in levels.iter() {
        let grouped_price = round_to_side_step(*price_units, side, tick_step_units);
        *grouped.entry(grouped_price).or_insert(0.0) += *qty;
    }

    grouped
}

/// Round price to tick step with side bias
///
/// Bids: floor (round down toward lower prices)
/// Asks: ceil (round up toward higher prices)
///
/// This ensures bids and asks don't overlap at bin edges.
fn round_to_side_step(price_units: i64, side: DepthSide, step_units: i64) -> i64 {
    if step_units <= 1 {
        return price_units;
    }

    match side {
        DepthSide::Bid => floor_to_step(price_units, step_units),
        DepthSide::Ask => ceil_to_step(price_units, step_units),
    }
}

/// Floor to multiple of step
fn floor_to_step(price_units: i64, step_units: i64) -> i64 {
    (price_units.div_euclid(step_units)) * step_units
}

/// Ceil to multiple of step
fn ceil_to_step(price_units: i64, step_units: i64) -> i64 {
    let floored = floor_to_step(price_units, step_units);
    if floored == price_units {
        price_units
    } else {
        floored + step_units
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_floor_to_step() {
        assert_eq!(floor_to_step(105, 10), 100);
        assert_eq!(floor_to_step(100, 10), 100);
        assert_eq!(floor_to_step(99, 10), 90);
        assert_eq!(floor_to_step(-105, 10), -110);
    }

    #[test]
    fn test_ceil_to_step() {
        assert_eq!(ceil_to_step(105, 10), 110);
        assert_eq!(ceil_to_step(100, 10), 100);
        assert_eq!(ceil_to_step(99, 10), 100);
        assert_eq!(ceil_to_step(-105, 10), -100);
    }

    #[test]
    fn test_group_bids() {
        let mut levels = BTreeMap::new();
        levels.insert(100, 10.0);
        levels.insert(105, 5.0);
        levels.insert(110, 8.0);

        let grouped = group_depth_by_tick(&levels, DepthSide::Bid, 10);

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped.get(&100), Some(&15.0)); // 100 + 105 -> 100
        assert_eq!(grouped.get(&110), Some(&8.0));
    }

    #[test]
    fn test_group_asks() {
        let mut levels = BTreeMap::new();
        levels.insert(100, 10.0);
        levels.insert(105, 5.0);
        levels.insert(110, 8.0);

        let grouped = group_depth_by_tick(&levels, DepthSide::Ask, 10);

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped.get(&100), Some(&10.0));
        assert_eq!(grouped.get(&110), Some(&13.0)); // 105 + 110 -> 110
    }

    #[test]
    fn test_no_grouping_with_step_one() {
        let mut levels = BTreeMap::new();
        levels.insert(100, 10.0);
        levels.insert(101, 5.0);

        let grouped = group_depth_by_tick(&levels, DepthSide::Bid, 1);

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped.get(&100), Some(&10.0));
        assert_eq!(grouped.get(&101), Some(&5.0));
    }

    #[test]
    fn test_empty_levels() {
        let levels = BTreeMap::new();
        let grouped = group_depth_by_tick(&levels, DepthSide::Bid, 10);
        assert!(grouped.is_empty());
    }

    #[test]
    fn test_side_bias_prevents_overlap() {
        // At boundary, bid floors down, ask ceils up
        let mut bid_levels = BTreeMap::new();
        bid_levels.insert(105, 10.0);

        let mut ask_levels = BTreeMap::new();
        ask_levels.insert(105, 10.0);

        let bid_grouped = group_depth_by_tick(&bid_levels, DepthSide::Bid, 10);
        let ask_grouped = group_depth_by_tick(&ask_levels, DepthSide::Ask, 10);

        // Bid rounds to 100, Ask rounds to 110 - no overlap
        assert_eq!(bid_grouped.get(&100), Some(&10.0));
        assert_eq!(ask_grouped.get(&110), Some(&10.0));
    }
}
