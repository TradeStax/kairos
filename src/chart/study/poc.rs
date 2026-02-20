//! Point of Control (POC) Study
//!
//! The Point of Control is the price level with the highest traded volume
//! within a given period.

use super::Footprint;
use exchange::util::Price;

/// POC configuration
#[derive(Debug, Clone, Copy)]
pub struct PocConfig {
    /// Whether to show POC line
    pub show_line: bool,
    /// Line width
    pub line_width: f32,
    /// Line alpha
    pub alpha: f32,
}

impl Default for PocConfig {
    fn default() -> Self {
        Self {
            show_line: true,
            line_width: 2.0,
            alpha: 0.8,
        }
    }
}

/// Find the Point of Control (highest volume price) in a footprint
pub fn find_poc(footprint: &Footprint) -> Option<(Price, f32)> {
    footprint
        .iter()
        .max_by(|(_, a), (_, b)| {
            a.total_qty()
                .partial_cmp(&b.total_qty())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(price, group)| (*price, group.total_qty()))
}

/// Find POC from a slice of trades
pub fn find_poc_from_trades<'a, I>(
    trades: I,
    tick_size: exchange::util::PriceStep,
) -> Option<(Price, f32)>
where
    I: Iterator<Item = &'a data::Trade>,
{
    use std::collections::BTreeMap;

    let mut volume_profile: BTreeMap<Price, f32> = BTreeMap::new();

    for trade in trades {
        let price_rounded = Price::from_units(trade.price.units()).round_to_step(tick_size);
        *volume_profile.entry(price_rounded).or_insert(0.0) += trade.quantity.0 as f32;
    }

    volume_profile
        .iter()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(price, volume)| (*price, *volume))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::study::TradeGroup;

    #[test]
    fn test_find_poc() {
        let mut footprint = Footprint::new();
        footprint.insert(Price::from_f32(100.0), TradeGroup::new(10.0, 5.0));
        footprint.insert(
            Price::from_f32(101.0),
            TradeGroup::new(20.0, 15.0), // 35 total - highest
        );
        footprint.insert(Price::from_f32(102.0), TradeGroup::new(5.0, 10.0));

        let (poc_price, poc_volume) = find_poc(&footprint).unwrap();
        assert_eq!(poc_price, Price::from_f32(101.0));
        assert_eq!(poc_volume, 35.0);
    }
}
