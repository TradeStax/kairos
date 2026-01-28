//! Trade rendering for heatmap chart
//!
//! This module contains functions for rendering trade markers
//! in both sparse (circles) and dense (rectangles) modes.

use crate::chart::ViewState;
use super::data::HeatmapData;
use data::Price as DataPrice;
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Path};
use iced::Point;

/// Maximum trade circle radius
pub const MAX_CIRCLE_RADIUS: f32 = 16.0;

/// Trade density thresholds for auto mode
pub const SPARSE_MODE_THRESHOLD: usize = 1_000;
/// Hard limit on draw calls per frame
pub const MAX_RENDER_BUDGET: usize = 10_000;

/// Trade rendering mode for heatmap
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeRenderingMode {
    /// Render individual trade circles (only for low trade count)
    Sparse,
    /// Render aggregated rectangles (for high trade density)
    Dense,
    /// Automatically switch based on visible trade count
    Auto,
}

impl Default for TradeRenderingMode {
    fn default() -> Self {
        TradeRenderingMode::Auto
    }
}

/// Render trades in sparse mode (individual circles)
#[allow(clippy::too_many_arguments)]
pub fn render_sparse_trades(
    frame: &mut canvas::Frame,
    chart: &ViewState,
    heatmap_data: &HeatmapData,
    palette: &Extended,
    earliest: u64,
    latest: u64,
    trade_size_filter: f32,
    trade_size_scale: Option<u16>,
    max_trade_qty: f32,
    cell_height: f32,
    max_markers: usize,
    decimation_factor: usize,
) {
    let mut rendered_count = 0;
    let mut trade_index = 0;

    // VIEWPORT CULLING: BTreeMap::range() efficiently filters to visible time range
    for (time, dp) in heatmap_data.trades_by_time.range(earliest..=latest) {
        // Early termination if budget exhausted
        if rendered_count >= max_markers {
            break;
        }

        let x_position = chart.interval_to_x(*time);

        for trade in &dp.grouped_trades {
            // Apply LOD decimation - only render every Nth trade
            if decimation_factor > 1 && trade_index % decimation_factor != 0 {
                trade_index += 1;
                continue;
            }

            // Early termination if budget exhausted
            if rendered_count >= max_markers {
                break;
            }

            // Filter by trade size
            if trade.qty > trade_size_filter {
                let y_position = chart.price_to_y(
                    exchange::util::Price::from_units(trade.price.to_units())
                );

                let color = if trade.is_sell {
                    palette.danger.base.color
                } else {
                    palette.success.base.color
                };

                let radius = {
                    if let Some(scale) = trade_size_scale {
                        let scale_factor = (scale as f32) / 100.0;
                        1.0 + (trade.qty / max_trade_qty)
                            * (MAX_CIRCLE_RADIUS - 1.0)
                            * scale_factor
                    } else {
                        cell_height / 2.0
                    }
                };

                frame.fill(
                    &Path::circle(Point::new(x_position, y_position), radius),
                    color,
                );

                rendered_count += 1;
            }

            trade_index += 1;
        }

        if rendered_count >= max_markers {
            break;
        }
    }
}

/// Render trades in dense mode (aggregated rectangles)
#[allow(clippy::too_many_arguments)]
pub fn render_dense_trades(
    frame: &mut canvas::Frame,
    chart: &ViewState,
    heatmap_data: &HeatmapData,
    palette: &Extended,
    earliest: u64,
    latest: u64,
    highest: DataPrice,
    lowest: DataPrice,
    trade_size_filter: f32,
    max_trade_qty: f32,
    cell_height: f32,
) {
    let highest_units = highest.to_units();
    let lowest_units = lowest.to_units();

    // VIEWPORT CULLING: BTreeMap::range() for time filtering
    for (time, dp) in heatmap_data.trades_by_time.range(earliest..=latest) {
        let x_position = chart.interval_to_x(*time);
        let half_cell_width = (chart.cell_width / 2.0) * 0.8;

        // Render trades grouped by price level
        for trade in &dp.grouped_trades {
            // Filter by trade size
            if trade.qty <= trade_size_filter {
                continue;
            }

            // VIEWPORT CULLING: Skip trades outside visible price range
            let trade_price_units = trade.price.to_units();
            if trade_price_units < lowest_units || trade_price_units > highest_units {
                continue;
            }

            let y_position = chart.price_to_y(
                exchange::util::Price::from_units(trade.price.to_units())
            );

            let color = if trade.is_sell {
                palette.danger.base.color
            } else {
                palette.success.base.color
            };

            // Use quantity to determine alpha (larger trades = more opaque)
            let alpha = (0.3 + (trade.qty / max_trade_qty) * 0.7).min(1.0);

            // Draw rectangle instead of circle (more efficient, shows density)
            frame.fill_rectangle(
                Point::new(x_position - half_cell_width, y_position - (cell_height / 2.0)),
                iced::Size::new(half_cell_width * 2.0, cell_height),
                color.scale_alpha(alpha),
            );
        }
    }
}
