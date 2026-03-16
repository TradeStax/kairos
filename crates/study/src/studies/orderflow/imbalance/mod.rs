//! Imbalance study — diagonal bid/ask volume imbalance detection.
//!
//! Highlights price levels where there is a significant imbalance between
//! buying and selling pressure by comparing diagonal bid/ask volumes at
//! adjacent price levels within each candle's footprint profile.
//!
//! Each detected imbalance is emitted as a [`PriceLevel`] ray extending
//! rightward from the detection candle. Subsequent candles whose high-low
//! range includes the level price count as "hits", each multiplying the
//! ray's opacity by the `hit_decay` factor. Levels that fade below
//! `MIN_OPACITY` are pruned from the output, and total output is capped
//! at `MAX_OUTPUT_LEVELS` to bound renderer draw calls.

mod params;

#[cfg(test)]
mod tests;

use params::*;

use crate::config::StudyConfig;
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{PriceLevel, StudyOutput};
use crate::studies::orderflow::shared::profile_core;
use crate::util::candle_key;

/// Type and strength of imbalance detected at a price level.
#[derive(Debug, Clone, Copy)]
pub enum ImbalanceType {
    /// Buy imbalance: diagonal buy volume exceeds sell volume by `ratio`.
    Buy { ratio: f32 },
    /// Sell imbalance: sell volume exceeds diagonal buy volume by `ratio`.
    Sell { ratio: f32 },
}

/// Check if there's an imbalance between two price levels.
///
/// Compares sell quantity at one level against diagonal buy quantity
/// at the next higher level.
pub fn check_imbalance(
    sell_qty: f32,
    diagonal_buy_qty: f32,
    threshold: f32,
    ignore_zeros: bool,
) -> Option<ImbalanceType> {
    if ignore_zeros && (sell_qty <= 0.0 || diagonal_buy_qty <= 0.0) {
        return None;
    }

    if diagonal_buy_qty >= sell_qty && sell_qty > 0.0 {
        let ratio = diagonal_buy_qty / sell_qty;
        if ratio >= threshold {
            return Some(ImbalanceType::Buy { ratio });
        }
    }

    if sell_qty >= diagonal_buy_qty && diagonal_buy_qty > 0.0 {
        let ratio = sell_qty / diagonal_buy_qty;
        if ratio >= threshold {
            return Some(ImbalanceType::Sell { ratio });
        }
    }

    None
}

/// Maximum number of hits before `base_opacity * decay^n < MIN_OPACITY`.
///
/// Used as an early-exit bound in the hit-counting loop so we never
/// scan more candles than necessary.
fn max_visible_hits(base_opacity: f32, decay: f32) -> u32 {
    if base_opacity <= MIN_OPACITY {
        return 0;
    }
    if decay >= 1.0 {
        return u32::MAX;
    }
    if decay <= 0.0 {
        return 1;
    }
    let n = (MIN_OPACITY / base_opacity).ln() / decay.ln();
    (n.ceil() as u32).max(1)
}

/// Detects diagonal bid/ask imbalances within each candle's footprint
/// and renders them as decaying price-level rays.
///
/// For each candle, builds a footprint profile and performs the diagonal
/// comparison: sell volume at price level `i` versus buy volume at level
/// `i + 1`. When the ratio exceeds the configured threshold, a
/// [`PriceLevel`] ray is emitted starting at the detection candle.
/// Subsequent candles whose high-low range covers the level count as
/// "hits", each multiplying the ray's opacity by `hit_decay`. Levels
/// that fade below `MIN_OPACITY` are pruned, and total output is
/// capped at `MAX_OUTPUT_LEVELS`.
pub struct ImbalanceStudy {
    /// Persisted user-configurable parameter values.
    config: StudyConfig,
    /// Most recently computed output (levels or empty).
    output: StudyOutput,
    /// Schema of user-adjustable parameters for the settings UI.
    params: Vec<crate::config::ParameterDef>,
    /// Consolidated metadata: name, category, placement, capabilities.
    metadata: StudyMetadata,
}

impl ImbalanceStudy {
    /// Create a new imbalance study with default parameters.
    pub fn new() -> Self {
        let params = make_params();
        let config = StudyConfig::from_params("imbalance", &params);

        Self {
            config,
            output: StudyOutput::Empty,
            params,
            metadata: StudyMetadata {
                name: "Imbalance".into(),
                category: StudyCategory::OrderFlow,
                placement: StudyPlacement::Background,
                description: "Price levels with significant buy/sell imbalance".into(),
                config_version: 1,
                capabilities: StudyCapabilities {
                    needs_trades: true,
                    ..StudyCapabilities::default()
                },
            },
        }
    }
}

impl Default for ImbalanceStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for ImbalanceStudy {
    crate::impl_study_base!("imbalance");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let threshold = self.config.get_float("threshold", DEFAULT_THRESHOLD) as f32;
        let buy_color = self.config.get_color("buy_color", DEFAULT_BUY_COLOR);
        let sell_color = self.config.get_color("sell_color", DEFAULT_SELL_COLOR);
        let ignore_zeros = self.config.get_bool("ignore_zeros", true);
        let hit_decay = self.config.get_float("hit_decay", DEFAULT_HIT_DECAY) as f32;

        if input.candles.is_empty() || input.tick_size.units() <= 0 {
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        let total = input.candles.len();
        let tick_units = input.tick_size.units();

        // Pre-extract candle ranges as contiguous f64 pairs for
        // cache-friendly hit scanning (avoids repeated to_f64 calls).
        let ranges: Vec<(f64, f64)> = input
            .candles
            .iter()
            .map(|c| (c.low.to_f64(), c.high.to_f64()))
            .collect();

        // Early-exit bound: once a level accumulates this many hits
        // its opacity is guaranteed below MIN_OPACITY.
        let worst_base = buy_color.a.max(sell_color.a);
        let hit_limit = max_visible_hits(worst_base, hit_decay);

        let mut levels = Vec::new();

        for (ci, candle) in input.candles.iter().enumerate() {
            let profile = profile_core::build_profile_from_candles(
                std::slice::from_ref(candle),
                input.tick_size,
                tick_units,
            );

            if profile.len() < 2 {
                continue;
            }

            let key = candle_key(candle, ci, total, &input.basis);

            for i in 0..profile.len() - 1 {
                let imb = check_imbalance(
                    profile[i].sell_volume,
                    profile[i + 1].buy_volume,
                    threshold,
                    ignore_zeros,
                );
                let Some(imb) = imb else { continue };

                let (price, is_buy) = match imb {
                    ImbalanceType::Buy { .. } => (profile[i + 1].price, true),
                    ImbalanceType::Sell { .. } => (profile[i].price, false),
                };

                let base_opacity = if is_buy { buy_color.a } else { sell_color.a };

                // Count subsequent candles whose range covers this price,
                // stopping early once enough hits guarantee invisibility.
                let mut hits = 0u32;
                for &(low, high) in &ranges[ci + 1..] {
                    if low <= price && price <= high {
                        hits += 1;
                        if hits >= hit_limit {
                            break;
                        }
                    }
                }

                let opacity = base_opacity * hit_decay.powi(hits as i32);
                if opacity < MIN_OPACITY {
                    continue;
                }

                let color = if is_buy { buy_color } else { sell_color };

                levels.push(
                    PriceLevel::horizontal(price, "", color)
                        .with_opacity(opacity)
                        .with_start_x(key)
                        .without_label(),
                );
            }
        }

        // Cap output — keep the newest (rightmost) levels.
        if levels.len() > MAX_OUTPUT_LEVELS {
            levels.drain(..levels.len() - MAX_OUTPUT_LEVELS);
        }

        self.output = if levels.is_empty() {
            StudyOutput::Empty
        } else {
            StudyOutput::Levels(levels)
        };
        Ok(StudyResult::ok())
    }
}
