//! Footprint computation and output building helpers.
//!
//! Contains typed config accessors (mode, data type, scaling, etc.),
//! tick grouping quantum calculation, trade aggregation per candle,
//! and `FootprintData` output construction from internal state.

use crate::config::ParameterValue;
use crate::output::{
    BackgroundColorMode, FootprintCandle, FootprintCandlePosition, FootprintData,
    FootprintDataType, FootprintGroupingMode, FootprintLevel, FootprintRenderMode,
    FootprintScaling, OutsideBarStyle, StudyOutput, TextFormat,
};
use data::{Candle, SerializableColor, Side, Trade};
use std::collections::BTreeMap;

use super::FootprintStudy;

impl FootprintStudy {
    // ── Typed accessors ─────────────────────────────────────────────

    /// Parse the render mode from config ("Box" or "Profile").
    pub(super) fn mode(&self) -> FootprintRenderMode {
        match self.config.get_choice("mode", "Profile") {
            "Box" => FootprintRenderMode::Box,
            _ => FootprintRenderMode::Profile,
        }
    }

    /// Parse the data display type from config.
    pub(super) fn data_type(&self) -> FootprintDataType {
        match self.config.get_choice("data_type", "Volume") {
            "Bid/Ask Split" => FootprintDataType::BidAskSplit,
            "Delta" => FootprintDataType::Delta,
            "Delta + Volume" => FootprintDataType::DeltaAndVolume,
            _ => FootprintDataType::Volume,
        }
    }

    /// Parse the bar width scaling method from config.
    pub(super) fn scaling(&self) -> FootprintScaling {
        match self.config.get_choice("scaling", "Square Root") {
            "Linear" => FootprintScaling::Linear,
            "Logarithmic" => FootprintScaling::Log,
            "Visible Range" => FootprintScaling::VisibleRange,
            "Datapoint" => FootprintScaling::Datapoint,
            "Hybrid" => FootprintScaling::Hybrid { weight: 0.5 },
            _ => FootprintScaling::Sqrt,
        }
    }

    /// Parse the candle body marker alignment from config.
    pub(super) fn marker_alignment(&self) -> FootprintCandlePosition {
        match self.config.get_choice("marker_alignment", "Left") {
            "None" => FootprintCandlePosition::None,
            "Center" => FootprintCandlePosition::Center,
            "Right" => FootprintCandlePosition::Right,
            _ => FootprintCandlePosition::Left,
        }
    }

    /// Parse the outside bar marker style from config.
    pub(super) fn outside_bar_style(&self) -> OutsideBarStyle {
        match self.config.get_choice("outside_bar_style", "Body") {
            "Candle" => OutsideBarStyle::Candle,
            "None" => OutsideBarStyle::None,
            _ => OutsideBarStyle::Body,
        }
    }

    /// Parse the numeric text display format from config.
    pub(super) fn text_format(&self) -> TextFormat {
        match self.config.get_choice("text_format", "Automatic") {
            "Normal" => TextFormat::Normal,
            "K" => TextFormat::K,
            _ => TextFormat::Automatic,
        }
    }

    /// Parse the background cell coloring mode from config.
    pub(super) fn bg_color_mode(&self) -> BackgroundColorMode {
        match self.config.get_choice("bg_color_mode", "Volume Intensity") {
            "Delta Intensity" => BackgroundColorMode::DeltaIntensity,
            "None" => BackgroundColorMode::None,
            _ => BackgroundColorMode::VolumeIntensity,
        }
    }

    // ── Tick grouping ───────────────────────────────────────────────

    /// Compute the grouping quantum for a candle (in price units).
    ///
    /// - **Automatic**: 1-tick resolution; the renderer merges
    ///   dynamically based on y-axis zoom.
    /// - **Manual**: pre-grouped at `manual_ticks`, using either
    ///   Fixed (uniform) or Bar-based (per-candle) grouping.
    pub(super) fn quantum_for_candle(
        &self,
        tick_units: i64,
        candle_high: i64,
        candle_low: i64,
    ) -> i64 {
        let base = tick_units.max(1);
        match self.config.get_choice("auto_grouping", "Automatic") {
            "Manual" => {
                let manual = self.config.get_int("manual_ticks", 1).max(1);
                match self.config.get_choice("group_mode", "Bar-based") {
                    "Fixed" => base * manual,
                    _ => {
                        let range = (candle_high - candle_low).max(0);
                        let divisor = base * manual;
                        if divisor <= 0 || range <= 0 {
                            return base;
                        }
                        let levels = (range / divisor).max(1);
                        let group_ticks = (range / levels / base).max(1);
                        group_ticks * base
                    }
                }
            }
            // Automatic: 1-tick resolution
            _ => base,
        }
    }

    // ── Output building ─────────────────────────────────────────────

    /// Build `FootprintData` from internal state + current parameters.
    pub(super) fn build_output(&self, candles: &[Candle]) -> StudyOutput {
        if candles.is_empty() || self.candle_levels.is_empty() {
            return StudyOutput::Empty;
        }

        let mut fp_candles = Vec::with_capacity(candles.len());

        for (idx, candle) in candles.iter().enumerate() {
            let quantum = self.candle_quantums.get(idx).copied().unwrap_or(1).max(1);
            let levels_map = self.candle_levels.get(idx);
            let (levels, poc_index) = match levels_map {
                Some(map) if !map.is_empty() => {
                    // Fill ALL price levels from candle low to high
                    let low = candle.low.units();
                    let high = candle.high.units();
                    let range_low = (low / quantum) * quantum;
                    let range_high = (high / quantum) * quantum;

                    let mut levels = Vec::new();
                    let mut price = range_low;
                    while price <= range_high {
                        let &(buy, sell) = map.get(&price).unwrap_or(&(0.0, 0.0));
                        levels.push(FootprintLevel {
                            price,
                            buy_volume: buy,
                            sell_volume: sell,
                        });
                        price += quantum;
                    }

                    // If somehow empty (shouldn't happen), fall
                    // back to trade-only levels
                    if levels.is_empty() {
                        let fallback: Vec<FootprintLevel> = map
                            .iter()
                            .map(|(&p, &(b, s))| FootprintLevel {
                                price: p,
                                buy_volume: b,
                                sell_volume: s,
                            })
                            .collect();
                        let poc = fallback
                            .iter()
                            .enumerate()
                            .max_by(|(_, a), (_, b)| {
                                a.total_qty()
                                    .partial_cmp(&b.total_qty())
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|(i, _)| i);
                        (fallback, poc)
                    } else {
                        let poc = levels
                            .iter()
                            .enumerate()
                            .max_by(|(_, a), (_, b)| {
                                a.total_qty()
                                    .partial_cmp(&b.total_qty())
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|(i, _)| i);
                        (levels, poc)
                    }
                }
                _ => (Vec::new(), None),
            };

            fp_candles.push(FootprintCandle {
                x: candle.time.0,
                open: candle.open.units(),
                high: candle.high.units(),
                low: candle.low.units(),
                close: candle.close.units(),
                levels,
                poc_index,
                quantum,
            });
        }

        let grouping_mode = match self.config.get_choice("auto_grouping", "Automatic") {
            "Manual" => FootprintGroupingMode::Manual,
            _ => FootprintGroupingMode::Automatic {
                factor: self.config.get_int("auto_group_factor", 1).max(1),
            },
        };

        let color_or_none = |key: &str| -> Option<SerializableColor> {
            self.config.get(key).and_then(|v| match v {
                ParameterValue::Color(c) if c.a > 0.0 => Some(*c),
                _ => None,
            })
        };
        let bg_buy_color = color_or_none("bg_buy_color");
        let bg_sell_color = color_or_none("bg_sell_color");
        let text_color = color_or_none("text_color");

        StudyOutput::Footprint(FootprintData {
            mode: self.mode(),
            data_type: self.data_type(),
            scaling: self.scaling(),
            candle_position: self.marker_alignment(),
            candles: fp_candles,
            bar_marker_width: self.config.get_float("bar_marker_width", 0.25) as f32,
            outside_bar_style: self.outside_bar_style(),
            show_outside_border: self.config.get_bool("show_outside_border", false),
            max_bars_to_show: self.config.get_int("max_bars_to_show", 200) as usize,
            bg_color_mode: self.bg_color_mode(),
            bg_max_alpha: self.config.get_float("bg_max_alpha", 0.6) as f32,
            bg_buy_color,
            bg_sell_color,
            show_grid_lines: self.config.get_bool("show_grid_lines", true),
            font_size: self.config.get_float("font_size", 11.0) as f32,
            text_format: self.text_format(),
            dynamic_text_size: self.config.get_bool("dynamic_text_size", true),
            show_zero_values: self.config.get_bool("show_zero_values", false),
            text_color,
            grouping_mode,
        })
    }

    /// Aggregate trades into the candle_levels map for a given candle
    /// index.
    pub(super) fn aggregate_trades_for_candle(
        &mut self,
        candle_idx: usize,
        trades: &[Trade],
        group_quantum: i64,
    ) {
        if candle_idx >= self.candle_levels.len() {
            self.candle_levels
                .resize_with(candle_idx + 1, BTreeMap::new);
        }
        let map = &mut self.candle_levels[candle_idx];
        for trade in trades {
            let price_units = trade.price.units();
            let rounded = if group_quantum > 0 {
                (price_units / group_quantum) * group_quantum
            } else {
                price_units
            };
            let entry = map.entry(rounded).or_insert((0.0, 0.0));
            match trade.side {
                Side::Buy | Side::Bid => {
                    entry.0 += trade.quantity.0 as f32;
                }
                Side::Sell | Side::Ask => {
                    entry.1 += trade.quantity.0 as f32;
                }
            }
        }
    }
}
