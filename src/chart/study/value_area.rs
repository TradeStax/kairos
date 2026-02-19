//! Value Area calculation (High/Low representing ~70% of volume)

use super::Footprint;

/// Value area result
#[allow(dead_code)]
pub struct ValueArea {
    pub vah: f32,
    pub val: f32,
    pub poc: f32,
}

/// Calculate value area from a footprint
#[allow(dead_code)]
pub fn calculate_value_area(footprint: &Footprint, percentage: f32) -> Option<ValueArea> {
    if footprint.is_empty() {
        return None;
    }

    let total_volume: f32 = footprint.values().map(|tg| tg.total_qty()).sum();
    let target_volume = total_volume * percentage;

    // Find POC (price with highest volume)
    let (poc_price, _) = footprint
        .iter()
        .max_by(|a, b| a.1.total_qty().partial_cmp(&b.1.total_qty()).unwrap())?;

    let poc = poc_price.to_f32_lossy();
    let mut accumulated = footprint.get(poc_price)?.total_qty();

    let prices: Vec<_> = footprint.keys().collect();
    let poc_idx = prices.iter().position(|p| *p == poc_price)?;

    let mut upper = poc_idx;
    let mut lower = poc_idx;

    while accumulated < target_volume && (lower > 0 || upper < prices.len() - 1) {
        let up_vol = if upper + 1 < prices.len() {
            footprint.get(prices[upper + 1]).map_or(0.0, |tg| tg.total_qty())
        } else {
            0.0
        };
        let down_vol = if lower > 0 {
            footprint.get(prices[lower - 1]).map_or(0.0, |tg| tg.total_qty())
        } else {
            0.0
        };

        if up_vol >= down_vol && upper + 1 < prices.len() {
            upper += 1;
            accumulated += up_vol;
        } else if lower > 0 {
            lower -= 1;
            accumulated += down_vol;
        } else if upper + 1 < prices.len() {
            upper += 1;
            accumulated += up_vol;
        } else {
            break;
        }
    }

    Some(ValueArea {
        vah: prices[upper].to_f32_lossy(),
        val: prices[lower].to_f32_lossy(),
        poc,
    })
}
