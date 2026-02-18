//! Imbalance Markers Study
//!
//! Highlights price levels where there's a significant imbalance between
//! buying and selling pressure at adjacent price levels.

use super::Footprint;
use exchange::util::Price;
use iced::theme::palette::Extended;
use iced::widget::canvas::Frame;
use iced::{Point, Size};

/// Imbalance configuration
#[derive(Debug, Clone, Copy)]
pub struct ImbalanceConfig {
    /// Minimum percentage threshold for imbalance (e.g., 300 = 300%)
    pub threshold: u8,
    /// Whether to use color scaling based on imbalance magnitude
    pub color_scale: bool,
    /// Whether to ignore zero quantity levels
    pub ignore_zeros: bool,
}

impl Default for ImbalanceConfig {
    fn default() -> Self {
        Self {
            threshold: 150,
            color_scale: true,
            ignore_zeros: true,
        }
    }
}

impl ImbalanceConfig {
    /// Create config from study parameters
    pub fn from_params(threshold: usize, color_scale: bool, ignore_zeros: bool) -> Self {
        Self {
            threshold: threshold as u8,
            color_scale,
            ignore_zeros,
        }
    }
}

/// Draw imbalance markers for a price level
#[allow(clippy::too_many_arguments)]
pub fn draw_imbalance_markers(
    frame: &mut Frame,
    price_to_y: &impl Fn(Price) -> f32,
    footprint: &Footprint,
    price: Price,
    sell_qty: f32,
    higher_price: Price,
    threshold: u8,
    color_scale: bool,
    ignore_zeros: bool,
    cell_height: f32,
    palette: &Extended,
    buyside_x: f32,
    sellside_x: f32,
    rect_width: f32,
) {
    if ignore_zeros && sell_qty <= 0.0 {
        return;
    }

    if let Some(group) = footprint.get(&higher_price) {
        let diagonal_buy_qty = group.buy_qty;

        if ignore_zeros && diagonal_buy_qty <= 0.0 {
            return;
        }

        let rect_height = cell_height / 2.0;

        let alpha_from_ratio = |ratio: f32| -> f32 {
            if color_scale {
                // Smooth color scale based on ratio
                (0.2 + 0.8 * (ratio - 1.0).min(1.0)).min(1.0)
            } else {
                1.0
            }
        };

        if diagonal_buy_qty >= sell_qty {
            let required_qty = sell_qty * (100 + threshold as u32) as f32 / 100.0;
            if diagonal_buy_qty > required_qty {
                let ratio = diagonal_buy_qty / required_qty;
                let alpha = alpha_from_ratio(ratio);

                let y = price_to_y(higher_price);
                frame.fill_rectangle(
                    Point::new(buyside_x, y - (rect_height / 2.0)),
                    Size::new(rect_width, rect_height),
                    palette.success.weak.color.scale_alpha(alpha),
                );
            }
        } else {
            let required_qty = diagonal_buy_qty * (100 + threshold as u32) as f32 / 100.0;
            if sell_qty > required_qty {
                let ratio = sell_qty / required_qty;
                let alpha = alpha_from_ratio(ratio);

                let y = price_to_y(price);
                frame.fill_rectangle(
                    Point::new(sellside_x, y - (rect_height / 2.0)),
                    Size::new(rect_width, rect_height),
                    palette.danger.weak.color.scale_alpha(alpha),
                );
            }
        }
    }
}

/// Check if there's an imbalance between two price levels
pub fn check_imbalance(
    _buy_qty: f32,
    sell_qty: f32,
    diagonal_buy_qty: f32,
    _diagonal_sell_qty: f32,
    threshold: u8,
    ignore_zeros: bool,
) -> Option<ImbalanceType> {
    if ignore_zeros && (sell_qty <= 0.0 || diagonal_buy_qty <= 0.0) {
        return None;
    }

    let threshold_mult = (100 + threshold as u32) as f32 / 100.0;

    // Check for buy imbalance (diagonal buy > current sell)
    if diagonal_buy_qty >= sell_qty {
        let required_qty = sell_qty * threshold_mult;
        if diagonal_buy_qty > required_qty {
            let ratio = diagonal_buy_qty / required_qty;
            return Some(ImbalanceType::Buy { ratio });
        }
    }

    // Check for sell imbalance (current sell > diagonal buy)
    if sell_qty >= diagonal_buy_qty {
        let required_qty = diagonal_buy_qty * threshold_mult;
        if sell_qty > required_qty {
            let ratio = sell_qty / required_qty;
            return Some(ImbalanceType::Sell { ratio });
        }
    }

    None
}

/// Type of imbalance detected
#[derive(Debug, Clone, Copy)]
pub enum ImbalanceType {
    /// Buying pressure imbalance
    Buy { ratio: f32 },
    /// Selling pressure imbalance
    Sell { ratio: f32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_imbalance() {
        // Buy imbalance: diagonal buy (200) > sell (50) by more than 150%
        let result = check_imbalance(100.0, 50.0, 200.0, 30.0, 150, false);
        assert!(matches!(result, Some(ImbalanceType::Buy { .. })));

        // No imbalance: diagonal buy (75) is not > sell (50) * 2.5
        let result = check_imbalance(100.0, 50.0, 75.0, 30.0, 150, false);
        assert!(result.is_none());

        // Sell imbalance: sell (200) > diagonal buy (50) by more than 150%
        let result = check_imbalance(100.0, 200.0, 50.0, 30.0, 150, false);
        assert!(matches!(result, Some(ImbalanceType::Sell { .. })));
    }

    #[test]
    fn test_ignore_zeros() {
        // With ignore_zeros, zero quantities should return None
        let result = check_imbalance(100.0, 0.0, 200.0, 30.0, 150, true);
        assert!(result.is_none());

        let result = check_imbalance(100.0, 50.0, 0.0, 30.0, 150, true);
        assert!(result.is_none());

        // Without ignore_zeros, still check normally
        let result = check_imbalance(100.0, 0.0, 200.0, 30.0, 150, false);
        assert!(matches!(result, Some(ImbalanceType::Buy { .. })));
    }
}
