//! Volume profile renderer
//!
//! Renders volume profiles as horizontal bars from the side of the chart.
//! Each `ProfileLevel` has buy and sell volume at a given price.
//! The POC (Point of Control) and value area boundaries are highlighted.

use crate::chart::ViewState;
use exchange::util::Price;
use iced::widget::canvas::Frame;
use iced::{Point, Size};
use study::output::{ProfileData, ProfileSide};

/// Render a volume profile.
pub fn render_profile(frame: &mut Frame, profile: &ProfileData, state: &ViewState, bounds: Size) {
    use crate::style::theme_bridge::rgba_to_iced_color;

    if profile.levels.is_empty() {
        return;
    }

    let buy_color = rgba_to_iced_color(profile.buy_color);
    let sell_color = rgba_to_iced_color(profile.sell_color);
    let poc_color = rgba_to_iced_color(profile.poc_color);
    let va_color = rgba_to_iced_color(profile.value_area_color);

    // Find the maximum total volume across all levels for normalization
    let max_volume = profile
        .levels
        .iter()
        .map(|l| l.buy_volume + l.sell_volume)
        .fold(0.0_f32, f32::max);

    if max_volume <= 0.0 {
        return;
    }

    // Maximum bar length as a fraction of chart width
    let max_bar_length = bounds.width * profile.width_pct;

    // Estimate bar height from adjacent price levels
    let bar_height = if profile.levels.len() >= 2 {
        let y0 = state.price_to_y(Price::from_units(profile.levels[0].price_units));
        let y1 = state.price_to_y(Price::from_units(profile.levels[1].price_units));
        (y1 - y0).abs().max(1.0)
    } else {
        state.cell_height.max(1.0)
    };

    // Draw value area background if present
    if let Some((vah_idx, val_idx)) = profile.value_area
        && let (Some(vah_level), Some(val_level)) =
            (profile.levels.get(vah_idx), profile.levels.get(val_idx))
    {
        let y_vah = state.price_to_y(Price::from_units(vah_level.price_units));
        let y_val = state.price_to_y(Price::from_units(val_level.price_units));

        let top = y_vah.min(y_val);
        let height = (y_vah - y_val).abs();

        if height > 0.0 {
            frame.fill_rectangle(
                Point::new(-bounds.width, top),
                Size::new(bounds.width * 3.0, height),
                va_color,
            );
        }
    }

    // Draw each level
    for (idx, level) in profile.levels.iter().enumerate() {
        let y = state.price_to_y(Price::from_units(level.price_units));
        let total = level.buy_volume + level.sell_volume;
        if total <= 0.0 {
            continue;
        }

        let bar_length = (total / max_volume) * max_bar_length;
        let is_poc = profile.poc == Some(idx);

        let sell_length = (level.sell_volume / total) * bar_length;
        let buy_length = (level.buy_volume / total) * bar_length;

        let top = y - bar_height / 2.0;

        match profile.side {
            ProfileSide::Left => {
                // Bars grow from left edge to the right
                let start_x = -bounds.width;
                if level.sell_volume > 0.0 {
                    frame.fill_rectangle(
                        Point::new(start_x, top),
                        Size::new(sell_length, bar_height),
                        sell_color,
                    );
                }
                if level.buy_volume > 0.0 {
                    frame.fill_rectangle(
                        Point::new(start_x + sell_length, top),
                        Size::new(buy_length, bar_height),
                        buy_color,
                    );
                }
            }
            ProfileSide::Right => {
                // Bars grow from right edge to the left
                let right_x = bounds.width;
                if level.buy_volume > 0.0 {
                    frame.fill_rectangle(
                        Point::new(right_x - buy_length, top),
                        Size::new(buy_length, bar_height),
                        buy_color,
                    );
                }
                if level.sell_volume > 0.0 {
                    frame.fill_rectangle(
                        Point::new(right_x - bar_length, top),
                        Size::new(sell_length, bar_height),
                        sell_color,
                    );
                }
            }
            ProfileSide::Both => {
                // Split: sells from left, buys from right
                let sell_bar = (level.sell_volume / max_volume) * max_bar_length;
                let buy_bar = (level.buy_volume / max_volume) * max_bar_length;

                if level.sell_volume > 0.0 {
                    frame.fill_rectangle(
                        Point::new(-bounds.width, top),
                        Size::new(sell_bar, bar_height),
                        sell_color,
                    );
                }
                if level.buy_volume > 0.0 {
                    frame.fill_rectangle(
                        Point::new(bounds.width - buy_bar, top),
                        Size::new(buy_bar, bar_height),
                        buy_color,
                    );
                }
            }
        }

        // Highlight POC
        if is_poc {
            frame.fill_rectangle(
                Point::new(-bounds.width, top),
                Size::new(bounds.width * 3.0, bar_height),
                poc_color.scale_alpha(0.15),
            );
        }
    }
}
