//! Value Area Study
//!
//! The Value Area represents the price range where a specified percentage
//! (typically 70%) of trading volume occurred.

use super::Footprint;
use exchange::util::Price;

/// Value Area configuration
#[derive(Debug, Clone, Copy)]
pub struct ValueAreaConfig {
    /// Percentage of volume to include (default 70%)
    pub percentage: f32,
    /// Whether to show VAH line
    pub show_vah: bool,
    /// Whether to show VAL line
    pub show_val: bool,
    /// Line alpha
    pub alpha: f32,
}

impl Default for ValueAreaConfig {
    fn default() -> Self {
        Self {
            percentage: 0.70,
            show_vah: true,
            show_val: true,
            alpha: 0.6,
        }
    }
}

/// Value Area result containing high, low, and POC
#[derive(Debug, Clone, Copy)]
pub struct ValueArea {
    /// Value Area High - top of value area
    pub vah: Price,
    /// Value Area Low - bottom of value area
    pub val: Price,
    /// Point of Control - price with highest volume
    pub poc: Price,
    /// Total volume in the value area
    pub volume: f32,
}

/// Calculate the Value Area from a footprint
///
/// The algorithm:
/// 1. Find the POC (price with highest volume)
/// 2. Expand outward from POC, adding price levels until
///    the specified percentage of total volume is reached
pub fn calculate_value_area(footprint: &Footprint, config: &ValueAreaConfig) -> Option<ValueArea> {
    if footprint.is_empty() {
        return None;
    }

    // Calculate total volume
    let total_volume: f32 = footprint.values().map(|g| g.total_qty()).sum();
    if total_volume <= 0.0 {
        return None;
    }

    let target_volume = total_volume * config.percentage;

    // Find POC
    let (poc_price, poc_volume) = footprint
        .iter()
        .max_by(|(_, a), (_, b)| {
            a.total_qty()
                .partial_cmp(&b.total_qty())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(p, g)| (*p, g.total_qty()))?;

    // Convert footprint to sorted vector for easier navigation
    let prices: Vec<(Price, f32)> = footprint
        .iter()
        .map(|(p, g)| (*p, g.total_qty()))
        .collect();

    // Find POC index
    let poc_idx = prices.iter().position(|(p, _)| *p == poc_price)?;

    let mut va_volume = poc_volume;
    let mut low_idx = poc_idx;
    let mut high_idx = poc_idx;

    // Expand outward from POC
    while va_volume < target_volume && (low_idx > 0 || high_idx < prices.len() - 1) {
        // Calculate volume for expanding up vs down
        let vol_above = if high_idx < prices.len() - 1 {
            // Look at next two levels above (or just one if at edge)
            let v1 = prices.get(high_idx + 1).map(|(_, v)| *v).unwrap_or(0.0);
            let v2 = prices.get(high_idx + 2).map(|(_, v)| *v).unwrap_or(0.0);
            v1 + v2
        } else {
            0.0
        };

        let vol_below = if low_idx > 0 {
            // Look at next two levels below
            let v1 = prices.get(low_idx - 1).map(|(_, v)| *v).unwrap_or(0.0);
            let v2 = if low_idx >= 2 {
                prices.get(low_idx - 2).map(|(_, v)| *v).unwrap_or(0.0)
            } else {
                0.0
            };
            v1 + v2
        } else {
            0.0
        };

        // Expand in direction with more volume
        if vol_above >= vol_below && high_idx < prices.len() - 1 {
            high_idx += 1;
            va_volume += prices[high_idx].1;
            if high_idx < prices.len() - 1 && va_volume < target_volume {
                high_idx += 1;
                va_volume += prices[high_idx].1;
            }
        } else if low_idx > 0 {
            low_idx -= 1;
            va_volume += prices[low_idx].1;
            if low_idx > 0 && va_volume < target_volume {
                low_idx -= 1;
                va_volume += prices[low_idx].1;
            }
        } else {
            // Can't expand anymore
            break;
        }
    }

    Some(ValueArea {
        vah: prices[high_idx].0,
        val: prices[low_idx].0,
        poc: poc_price,
        volume: va_volume,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::study::TradeGroup;

    #[test]
    fn test_calculate_value_area() {
        let mut footprint = Footprint::new();

        // Create a distribution with clear POC at 100.0
        footprint.insert(Price::from_f32(98.0), TradeGroup::new(10.0, 5.0));
        footprint.insert(Price::from_f32(99.0), TradeGroup::new(20.0, 15.0));
        footprint.insert(Price::from_f32(100.0), TradeGroup::new(50.0, 45.0)); // POC
        footprint.insert(Price::from_f32(101.0), TradeGroup::new(25.0, 20.0));
        footprint.insert(Price::from_f32(102.0), TradeGroup::new(15.0, 10.0));

        let config = ValueAreaConfig::default();
        let va = calculate_value_area(&footprint, &config).unwrap();

        assert_eq!(va.poc, Price::from_f32(100.0));
        // Value area should encompass the POC
        assert!(va.val <= va.poc);
        assert!(va.vah >= va.poc);
    }
}
