//! Volume Profile Study
//!
//! Displays the distribution of trading volume across price levels.

use super::{Footprint, TradeGroup};
use data::{Side, Trade};
use exchange::util::{Price, PriceStep};
use std::collections::BTreeMap;

/// Volume Profile configuration
#[derive(Debug, Clone, Copy)]
pub struct VolumeProfileConfig {
    /// Whether to show buy volume
    pub show_buys: bool,
    /// Whether to show sell volume
    pub show_sells: bool,
    /// Bar alpha
    pub alpha: f32,
    /// Maximum bar width as percentage of cell width
    pub max_width_pct: f32,
}

impl Default for VolumeProfileConfig {
    fn default() -> Self {
        Self {
            show_buys: true,
            show_sells: true,
            alpha: 0.6,
            max_width_pct: 0.9,
        }
    }
}

/// Build a volume profile from trades
pub fn build_volume_profile(
    trades: &[Trade],
    tick_size: PriceStep,
    price_filter: Option<(Price, Price)>,
) -> Footprint {
    let mut footprint = BTreeMap::new();

    for trade in trades {
        let price_rounded = Price::from_units(trade.price.units()).round_to_step(tick_size);

        // Apply price filter if specified
        if let Some((lowest, highest)) = price_filter
            && (price_rounded < lowest || price_rounded > highest) {
                continue;
            }

        let entry = footprint.entry(price_rounded).or_insert(TradeGroup {
            buy_qty: 0.0,
            sell_qty: 0.0,
        });

        match trade.side {
            Side::Buy | Side::Bid => entry.buy_qty += trade.quantity.0 as f32,
            Side::Sell | Side::Ask => entry.sell_qty += trade.quantity.0 as f32,
        }
    }

    footprint
}

/// Build volume profile for a time range using binary search
pub fn build_volume_profile_for_range(
    trades: &[Trade],
    start_time: u64,
    end_time: u64,
    tick_size: PriceStep,
    price_filter: Option<(Price, Price)>,
) -> Footprint {
    // Find start index using binary search
    let start_idx = trades
        .binary_search_by_key(&start_time, |t| t.time.0)
        .unwrap_or_else(|i| i);

    // Find end index using binary search
    let end_idx = trades[start_idx..]
        .binary_search_by_key(&end_time, |t| t.time.0)
        .map(|i| start_idx + i)
        .unwrap_or_else(|i| start_idx + i);

    build_volume_profile(&trades[start_idx..end_idx], tick_size, price_filter)
}

/// Statistics for a volume profile
#[derive(Debug, Clone, Copy, Default)]
pub struct VolumeProfileStats {
    /// Total buy volume
    pub total_buys: f32,
    /// Total sell volume
    pub total_sells: f32,
    /// Maximum volume at any single price
    pub max_volume: f32,
    /// Number of price levels
    pub level_count: usize,
    /// Price with highest volume (POC)
    pub poc_price: Option<Price>,
    /// Volume at POC
    pub poc_volume: f32,
}

impl VolumeProfileStats {
    /// Calculate statistics from a footprint
    pub fn from_footprint(footprint: &Footprint) -> Self {
        let mut stats = Self::default();
        let mut max_total = 0.0_f32;

        for (price, group) in footprint {
            stats.total_buys += group.buy_qty;
            stats.total_sells += group.sell_qty;

            let total = group.total_qty();
            stats.max_volume = stats.max_volume.max(total);

            if total > max_total {
                max_total = total;
                stats.poc_price = Some(*price);
                stats.poc_volume = total;
            }
        }

        stats.level_count = footprint.len();
        stats
    }

    /// Total volume (buys + sells)
    pub fn total_volume(&self) -> f32 {
        self.total_buys + self.total_sells
    }

    /// Delta (buys - sells)
    pub fn delta(&self) -> f32 {
        self.total_buys - self.total_sells
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{Quantity, Timestamp};

    fn make_trade(time: u64, price: f32, qty: f32, side: Side) -> Trade {
        Trade {
            time: Timestamp(time),
            price: data::Price::from_f32(price),
            quantity: Quantity(qty as f64),
            side,
        }
    }

    /// Build an exchange::util::Price from data::Price to match how
    /// build_volume_profile creates its BTreeMap keys (via Price::from_units).
    fn price_key(value: f32) -> Price {
        Price::from_units(data::Price::from_f32(value).units())
    }

    #[test]
    fn test_build_volume_profile() {
        let trades = vec![
            make_trade(1000, 100.0, 10.0, Side::Buy),
            make_trade(1001, 100.0, 5.0, Side::Sell),
            make_trade(1002, 101.0, 15.0, Side::Buy),
            make_trade(1003, 99.0, 8.0, Side::Sell),
        ];

        let tick_size = PriceStep::from_f32(1.0);
        let profile = build_volume_profile(&trades, tick_size, None);

        assert_eq!(profile.len(), 3);

        // Lookup keys must be built the same way as build_volume_profile:
        // Price::from_units(data_price.units()).round_to_step(tick_size)
        let level_100 = profile.get(&price_key(100.0)).unwrap();
        assert_eq!(level_100.buy_qty, 10.0);
        assert_eq!(level_100.sell_qty, 5.0);

        let level_101 = profile.get(&price_key(101.0)).unwrap();
        assert_eq!(level_101.buy_qty, 15.0);
        assert_eq!(level_101.sell_qty, 0.0);
    }

    #[test]
    fn test_volume_profile_stats() {
        let mut footprint = Footprint::new();
        footprint.insert(Price::from_f32(100.0), TradeGroup::new(10.0, 5.0));
        footprint.insert(Price::from_f32(101.0), TradeGroup::new(20.0, 15.0));
        footprint.insert(Price::from_f32(102.0), TradeGroup::new(5.0, 10.0));

        let stats = VolumeProfileStats::from_footprint(&footprint);

        assert_eq!(stats.total_buys, 35.0);
        assert_eq!(stats.total_sells, 30.0);
        assert_eq!(stats.total_volume(), 65.0);
        assert_eq!(stats.delta(), 5.0);
        assert_eq!(stats.level_count, 3);
        assert_eq!(stats.poc_price, Some(Price::from_f32(101.0)));
        assert_eq!(stats.poc_volume, 35.0);
    }
}
