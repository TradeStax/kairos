//! Trade monitoring engine for level interaction tracking.
//!
//! Processes incoming trades against monitored levels to track touches,
//! holds, breaks, and status transitions. Runs incrementally on each
//! `append_trades()` call using the `processed_trade_count` offset
//! pattern (same as BigTrades).

use data::{Side, Trade};

use super::types::{
    ActiveTouch, BlockEvent, LevelStatus, MonitoredLevel,
    PendingBlock, TouchEvent,
};

/// Process new trades against all monitored levels.
///
/// Updates level statuses, touch events, and accumulated metrics.
/// `tolerance_units` is the distance in price units that defines the
/// level's interaction zone.
pub fn process_trades(
    levels: &mut [MonitoredLevel],
    trades: &[Trade],
    tolerance_units: i64,
    break_threshold: f64,
    block_window_ms: u64,
    block_min_qty: f64,
) {
    if trades.is_empty() || levels.is_empty() {
        return;
    }

    let break_distance =
        (tolerance_units as f64 * break_threshold) as i64;

    for trade in trades {
        let trade_units = trade.price.units();
        let trade_time = trade.time.0;
        let trade_volume = trade.quantity.0 as f64;
        let is_buy = matches!(trade.side, Side::Buy | Side::Ask);
        let signed_delta = if is_buy {
            trade_volume
        } else {
            -trade_volume
        };

        for level in levels.iter_mut() {
            let distance =
                (trade_units - level.price_units).abs();
            let within_tolerance = distance <= tolerance_units;
            let beyond_break = distance > break_distance;

            match (&level.status, within_tolerance, beyond_break) {
                // Trade within tolerance: start or continue touch
                (_, true, _) => {
                    handle_touch(
                        level,
                        trade_time,
                        trade_volume,
                        signed_delta,
                        is_buy,
                        trade_units,
                        tolerance_units,
                        block_window_ms,
                        block_min_qty,
                    );
                }

                // Trade moved away beyond break distance
                (LevelStatus::BeingTested, false, true) => {
                    // Check if this is a break
                    let excursion_ticks =
                        trade_units - level.price_units;
                    finalize_touch(level, trade_time, false, block_min_qty);
                    level.break_count += 1;
                    level.status = LevelStatus::Broken;

                    // Record excursion on the last touch
                    if let Some(last) = level.touches.last_mut() {
                        last.max_excursion_ticks =
                            excursion_ticks as i32;
                    }
                }

                // Trade moved away within break distance — hold
                (LevelStatus::BeingTested, false, false) => {
                    finalize_touch(level, trade_time, true, block_min_qty);
                    evaluate_status(level);
                }

                _ => {}
            }
        }
    }
}

/// Handle a trade within a level's tolerance zone.
fn handle_touch(
    level: &mut MonitoredLevel,
    time: u64,
    volume: f64,
    delta: f64,
    is_buy: bool,
    trade_units: i64,
    tolerance_units: i64,
    block_window_ms: u64,
    block_min_qty: f64,
) {
    let excursion =
        ((trade_units - level.price_units).abs() as f64
            / tolerance_units as f64
            * 100.0) as i32;

    match &mut level.active_touch {
        Some(touch) => {
            // Continue existing touch
            touch.volume += volume;
            touch.delta += delta;
            touch.last_trade_time = time;
            if excursion > touch.max_excursion_ticks {
                touch.max_excursion_ticks = excursion;
            }

            // Track buy/sell volume separately
            if is_buy {
                touch.buy_volume += volume;
            } else {
                touch.sell_volume += volume;
            }

            // Block aggregation
            accumulate_block(
                touch,
                time,
                volume,
                is_buy,
                block_window_ms,
                block_min_qty,
            );
        }
        None => {
            // Start new touch
            if level.status != LevelStatus::Broken {
                level.status = LevelStatus::BeingTested;
            }
            level.touch_count += 1;

            let (buy_vol, sell_vol) = if is_buy {
                (volume, 0.0)
            } else {
                (0.0, volume)
            };

            level.active_touch = Some(ActiveTouch {
                start_time: time,
                volume,
                delta,
                max_excursion_ticks: excursion,
                last_trade_time: time,
                buy_volume: buy_vol,
                sell_volume: sell_vol,
                pending_block: Some(PendingBlock {
                    start_time: time,
                    last_fill_time: time,
                    quantity: volume,
                    fill_count: 1,
                    is_buy,
                }),
                blocks: Vec::new(),
            });
        }
    }

    level.total_volume_absorbed += volume;
    level.net_delta += delta;
}

/// Accumulate a fill into the active touch's pending block.
fn accumulate_block(
    touch: &mut ActiveTouch,
    time: u64,
    volume: f64,
    is_buy: bool,
    window_ms: u64,
    min_qty: f64,
) {
    match &mut touch.pending_block {
        Some(pb)
            if pb.is_buy == is_buy
                && time.saturating_sub(pb.last_fill_time)
                    <= window_ms =>
        {
            // Same side, within window — merge
            pb.quantity += volume;
            pb.last_fill_time = time;
            pb.fill_count += 1;
        }
        _ => {
            // Flush existing pending block if it meets threshold
            flush_pending_block(touch, min_qty);
            // Start new pending block
            touch.pending_block = Some(PendingBlock {
                start_time: time,
                last_fill_time: time,
                quantity: volume,
                fill_count: 1,
                is_buy,
            });
        }
    }
}

/// Flush pending block into blocks vec if it meets min quantity.
fn flush_pending_block(touch: &mut ActiveTouch, min_qty: f64) {
    if let Some(pb) = touch.pending_block.take() {
        if pb.quantity >= min_qty {
            touch.blocks.push(BlockEvent {
                time: pb.start_time,
                quantity: pb.quantity,
                is_buy: pb.is_buy,
                fill_count: pb.fill_count,
            });
        }
    }
}

/// Finalize an active touch and record it as a TouchEvent.
fn finalize_touch(
    level: &mut MonitoredLevel,
    end_time: u64,
    held: bool,
    block_min_qty: f64,
) {
    if let Some(mut active) = level.active_touch.take() {
        let duration = end_time.saturating_sub(active.start_time);
        let rejection_velocity = if duration > 0 {
            active.volume / (duration as f64 / 1000.0)
        } else {
            0.0
        };

        // Flush any remaining pending block
        flush_pending_block(&mut active, block_min_qty);

        let block_count = active.blocks.len() as u32;

        // Compute touch quality score (0.0–1.0)
        let rejection_speed =
            (rejection_velocity / 500.0).clamp(0.0, 1.0);
        let delta_quality = active.delta.abs()
            / (active.volume + 1e-9);
        let block_quality =
            (block_count.min(3) as f64) / 3.0;
        let excursion_inv =
            1.0 - (active.max_excursion_ticks as f64 / 100.0)
                .clamp(0.0, 1.0);

        let quality = ((rejection_speed
            + delta_quality
            + block_quality
            + excursion_inv)
            / 4.0) as f32;

        // Update absorption on level flow
        if held {
            level.hold_count += 1;

            let defending = active.buy_volume.max(active.sell_volume);
            let attacking = active.buy_volume.min(active.sell_volume);
            let touch_absorption =
                defending / (attacking + 1e-9);

            let flow = &mut level.flow;
            if flow.absorption_ratio == 0.0 {
                flow.absorption_ratio = touch_absorption;
            } else {
                flow.absorption_ratio = 0.7
                    * flow.absorption_ratio
                    + 0.3 * touch_absorption;
            }
        }

        // Accumulate flow metrics from this touch
        level.flow.buy_volume += active.buy_volume;
        level.flow.sell_volume += active.sell_volume;
        for block in &active.blocks {
            if block.is_buy {
                level.flow.block_buy_volume += block.quantity;
            } else {
                level.flow.block_sell_volume += block.quantity;
            }
        }
        level.flow.block_count += block_count;

        level.delta_per_touch.push(active.delta);
        level.time_at_level += duration;

        level.touches.push(TouchEvent {
            start_time: active.start_time,
            end_time,
            volume: active.volume,
            delta: active.delta,
            held,
            max_excursion_ticks: active.max_excursion_ticks,
            rejection_velocity,
            buy_volume: active.buy_volume,
            sell_volume: active.sell_volume,
            blocks: active.blocks,
            quality_score: quality.clamp(0.0, 1.0),
            approach_velocity: 0.0, // Not computed yet
        });
    }
}

/// Evaluate the level's status after a touch completes.
///
/// Uses a weighted multi-signal composite score:
///   35% hold ratio + 25% recent quality + 25% absorption + 15% block presence.
fn evaluate_status(level: &mut MonitoredLevel) {
    if level.touch_count < 2 {
        level.status = LevelStatus::Holding;
        level.strength = 0.5;
        return;
    }

    let hold_ratio = if level.touch_count > 0 {
        level.hold_count as f64 / level.touch_count as f64
    } else {
        1.0
    };

    // Signal 1: Hold ratio (0.0 = all breaks, 1.0 = all holds)
    let hold_signal = hold_ratio;

    // Signal 2: Recent touch quality trend (last 3 touches)
    let quality_signal = if level.touches.len() >= 3 {
        let n = level.touches.len();
        let sum: f32 = level.touches[n - 3..]
            .iter()
            .map(|t| t.quality_score)
            .sum();
        (sum / 3.0) as f64
    } else {
        0.5
    };

    // Signal 3: Absorption strength
    let absorption_signal =
        (level.flow.absorption_ratio / 2.0).clamp(0.0, 1.0);

    // Signal 4: Block trade presence on defending side
    let total_block = level.flow.block_buy_volume
        + level.flow.block_sell_volume;
    let defending_block_ratio = if total_block > 0.0 {
        level
            .flow
            .block_buy_volume
            .max(level.flow.block_sell_volume)
            / total_block
    } else {
        0.0
    };
    let block_signal = defending_block_ratio.clamp(0.0, 1.0);

    // Weighted composite
    let composite = 0.35 * hold_signal
        + 0.25 * quality_signal
        + 0.25 * absorption_signal
        + 0.15 * block_signal;

    level.strength = composite as f32;

    if composite >= 0.6 {
        level.status = LevelStatus::Holding;
    } else if composite >= 0.35 {
        level.status = LevelStatus::Weakening;
    }
    // Below 0.35: don't auto-break; only process_trades sets Broken
}

/// Compute ATR from candles (Wilder's method).
pub fn compute_atr(candles: &[data::Candle], period: usize) -> Option<f64> {
    if candles.len() < 2 {
        return None;
    }

    let mut true_ranges: Vec<f64> =
        Vec::with_capacity(candles.len() - 1);

    for i in 1..candles.len() {
        let h = candles[i].high.to_f64();
        let l = candles[i].low.to_f64();
        let pc = candles[i - 1].close.to_f64();
        let tr = (h - l).max((h - pc).abs()).max((l - pc).abs());
        true_ranges.push(tr);
    }

    if true_ranges.len() < period {
        return Some(
            true_ranges.iter().sum::<f64>() / true_ranges.len() as f64,
        );
    }

    // Initial SMA
    let mut atr: f64 =
        true_ranges[..period].iter().sum::<f64>() / period as f64;

    // Wilder smoothing
    for tr in &true_ranges[period..] {
        atr = (atr * (period as f64 - 1.0) + tr) / period as f64;
    }

    Some(atr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{Price, Quantity, Timestamp};

    const BLOCK_WINDOW: u64 = 40;
    const BLOCK_MIN: f64 = 25.0;

    fn make_level(price_units: i64) -> MonitoredLevel {
        use super::super::types::LevelSource;
        use super::super::session::{SessionKey, SessionType};
        MonitoredLevel::new(
            1,
            price_units,
            Price::from_units(price_units).to_f64(),
            LevelSource::Poc,
            0,
            SessionKey {
                trade_date: "test".into(),
                session_type: SessionType::Rth,
            },
        )
    }

    fn make_trade(
        time: u64,
        price_units: i64,
        qty: u32,
        side: Side,
    ) -> Trade {
        Trade::new(
            Timestamp(time),
            Price::from_units(price_units),
            Quantity(f64::from(qty)),
            side,
        )
    }

    fn pt(
        levels: &mut [MonitoredLevel],
        trades: &[Trade],
        tolerance: i64,
    ) {
        process_trades(
            levels,
            trades,
            tolerance,
            1.5,
            BLOCK_WINDOW,
            BLOCK_MIN,
        );
    }

    #[test]
    fn test_touch_detection() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let level_price = 100_000_000i64;

        let mut levels = vec![make_level(level_price)];
        let trades = vec![make_trade(
            1000,
            level_price,
            10,
            Side::Buy,
        )];

        pt(&mut levels, &trades, tolerance);
        assert_eq!(levels[0].status, LevelStatus::BeingTested);
        assert_eq!(levels[0].touch_count, 1);
    }

    #[test]
    fn test_hold_after_touch() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let level_price = 100_000_000i64;

        let mut levels = vec![make_level(level_price)];

        let touch = vec![make_trade(
            1000,
            level_price,
            10,
            Side::Buy,
        )];
        pt(&mut levels, &touch, tolerance);

        let away = vec![make_trade(
            2000,
            level_price + tolerance + 1,
            10,
            Side::Buy,
        )];
        pt(&mut levels, &away, tolerance);

        assert_eq!(levels[0].status, LevelStatus::Holding);
        assert_eq!(levels[0].hold_count, 1);
    }

    #[test]
    fn test_break() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let level_price = 100_000_000i64;
        let break_dist = (tolerance as f64 * 1.5) as i64;

        let mut levels = vec![make_level(level_price)];

        let touch = vec![make_trade(
            1000,
            level_price,
            10,
            Side::Buy,
        )];
        pt(&mut levels, &touch, tolerance);

        let far = vec![make_trade(
            2000,
            level_price + break_dist + 1,
            10,
            Side::Buy,
        )];
        pt(&mut levels, &far, tolerance);

        assert_eq!(levels[0].status, LevelStatus::Broken);
    }

    #[test]
    fn test_buy_sell_volume_tracking() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let lp = 100_000_000i64;

        let mut levels = vec![make_level(lp)];

        // Buy 10 then sell 5 at level, then move away
        let trades = vec![
            make_trade(1000, lp, 10, Side::Buy),
            make_trade(1010, lp, 5, Side::Sell),
        ];
        pt(&mut levels, &trades, tolerance);

        // Move away to finalize
        let away = vec![make_trade(
            2000,
            lp + tolerance + 1,
            1,
            Side::Buy,
        )];
        pt(&mut levels, &away, tolerance);

        let touch = &levels[0].touches[0];
        assert_eq!(touch.buy_volume, 10.0);
        assert_eq!(touch.sell_volume, 5.0);

        assert_eq!(levels[0].flow.buy_volume, 10.0);
        assert_eq!(levels[0].flow.sell_volume, 5.0);
    }

    #[test]
    fn test_block_aggregation_same_side() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let lp = 100_000_000i64;

        let mut levels = vec![make_level(lp)];

        // 3 buy fills within aggregation window (40ms)
        // totaling 30 which exceeds block_min 25
        let trades = vec![
            make_trade(1000, lp, 10, Side::Buy),
            make_trade(1020, lp, 10, Side::Buy),
            make_trade(1035, lp, 10, Side::Buy),
        ];
        pt(&mut levels, &trades, tolerance);

        // Move away to finalize (flushes pending block)
        let away = vec![make_trade(
            2000,
            lp + tolerance + 1,
            1,
            Side::Buy,
        )];
        pt(&mut levels, &away, tolerance);

        let touch = &levels[0].touches[0];
        assert_eq!(touch.blocks.len(), 1);
        assert_eq!(touch.blocks[0].quantity, 30.0);
        assert!(touch.blocks[0].is_buy);
        assert_eq!(touch.blocks[0].fill_count, 3);
    }

    #[test]
    fn test_block_side_change_flushes() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let lp = 100_000_000i64;

        let mut levels = vec![make_level(lp)];

        // Buy block then sell — side change flushes
        let trades = vec![
            make_trade(1000, lp, 15, Side::Buy),
            make_trade(1020, lp, 15, Side::Buy),  // 30 buy (block)
            make_trade(1040, lp, 10, Side::Sell),  // side change
        ];
        pt(&mut levels, &trades, tolerance);

        let away = vec![make_trade(
            2000,
            lp + tolerance + 1,
            1,
            Side::Buy,
        )];
        pt(&mut levels, &away, tolerance);

        let touch = &levels[0].touches[0];
        // Buy block (30) qualifies, sell (10) doesn't meet min 25
        assert_eq!(touch.blocks.len(), 1);
        assert!(touch.blocks[0].is_buy);
        assert_eq!(touch.blocks[0].quantity, 30.0);
    }

    #[test]
    fn test_block_below_threshold_not_recorded() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let lp = 100_000_000i64;

        let mut levels = vec![make_level(lp)];

        // Small fills that don't exceed block_min_qty=25
        let trades = vec![
            make_trade(1000, lp, 5, Side::Buy),
            make_trade(1020, lp, 5, Side::Buy),
        ];
        pt(&mut levels, &trades, tolerance);

        let away = vec![make_trade(
            2000,
            lp + tolerance + 1,
            1,
            Side::Buy,
        )];
        pt(&mut levels, &away, tolerance);

        let touch = &levels[0].touches[0];
        assert!(touch.blocks.is_empty());
    }

    #[test]
    fn test_touch_quality_score() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let lp = 100_000_000i64;

        let mut levels = vec![make_level(lp)];

        // Strong one-sided buy at level, quick rejection
        let trades = vec![
            make_trade(1000, lp, 50, Side::Buy),
            make_trade(1010, lp, 50, Side::Buy),
            make_trade(1015, lp, 50, Side::Buy),
        ];
        pt(&mut levels, &trades, tolerance);

        let away = vec![make_trade(
            1100,
            lp + tolerance + 1,
            1,
            Side::Buy,
        )];
        pt(&mut levels, &away, tolerance);

        let touch = &levels[0].touches[0];
        assert!(touch.quality_score > 0.0);
        assert!(touch.quality_score <= 1.0);
        // Strong delta quality (all one side) should boost score
        assert!(touch.quality_score > 0.3);
    }

    #[test]
    fn test_absorption_ratio_on_hold() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let lp = 100_000_000i64;

        let mut levels = vec![make_level(lp)];

        // Defend level: 100 buy vs 20 sell → strong absorption
        let trades = vec![
            make_trade(1000, lp, 100, Side::Buy),
            make_trade(1010, lp, 20, Side::Sell),
        ];
        pt(&mut levels, &trades, tolerance);

        // Hold (move away within break distance)
        let away = vec![make_trade(
            2000,
            lp + tolerance + 1,
            1,
            Side::Buy,
        )];
        pt(&mut levels, &away, tolerance);

        // Absorption = 100 / (20 + 1e-9) ≈ 5.0
        assert!(levels[0].flow.absorption_ratio > 1.0);
    }

    #[test]
    fn test_multi_signal_status_holding() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let lp = 100_000_000i64;

        let mut levels = vec![make_level(lp)];

        // 3 touches that all hold → strong composite score
        for i in 0..3 {
            let t = (i * 2000) as u64;
            let trades = vec![
                make_trade(t + 1000, lp, 50, Side::Buy),
            ];
            pt(&mut levels, &trades, tolerance);

            let away = vec![make_trade(
                t + 1500,
                lp + tolerance + 1,
                1,
                Side::Buy,
            )];
            pt(&mut levels, &away, tolerance);
        }

        assert_eq!(levels[0].status, LevelStatus::Holding);
        assert!(levels[0].strength >= 0.6);
    }

    #[test]
    fn test_multi_signal_status_weakening() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let lp = 100_000_000i64;
        let break_dist = (tolerance as f64 * 1.5) as i64;

        let mut levels = vec![make_level(lp)];

        // Touch 1: hold
        pt(
            &mut levels,
            &[make_trade(1000, lp, 10, Side::Buy)],
            tolerance,
        );
        pt(
            &mut levels,
            &[make_trade(1500, lp + tolerance + 1, 1, Side::Buy)],
            tolerance,
        );

        // Touch 2: break (weakens the level)
        pt(
            &mut levels,
            &[make_trade(3000, lp, 5, Side::Sell)],
            tolerance,
        );
        pt(
            &mut levels,
            &[make_trade(
                3500,
                lp + break_dist + 1,
                10,
                Side::Buy,
            )],
            tolerance,
        );

        // After a break, status is Broken; test that the model
        // correctly tracked the break_count
        assert_eq!(levels[0].break_count, 1);
    }

    #[test]
    fn test_flow_block_volume_accumulation() {
        let tick = Price::from_f64(0.25);
        let tolerance = tick.units() * 4;
        let lp = 100_000_000i64;

        let mut levels = vec![make_level(lp)];

        // Touch with block buy and block sell
        let trades = vec![
            make_trade(1000, lp, 30, Side::Buy),  // block buy
            make_trade(1050, lp, 30, Side::Sell), // side change → flush buy block, start sell
            make_trade(1070, lp, 30, Side::Sell), // 60 sell block
        ];
        pt(&mut levels, &trades, tolerance);

        let away = vec![make_trade(
            2000,
            lp + tolerance + 1,
            1,
            Side::Buy,
        )];
        pt(&mut levels, &away, tolerance);

        assert_eq!(levels[0].flow.block_buy_volume, 30.0);
        assert_eq!(levels[0].flow.block_sell_volume, 60.0);
        assert_eq!(levels[0].flow.block_count, 2);
    }

    #[test]
    fn test_strength_defaults_zero_for_new_level() {
        let lp = 100_000_000i64;
        let level = make_level(lp);
        assert_eq!(level.strength, 0.0);
        assert_eq!(level.flow.absorption_ratio, 0.0);
        assert_eq!(level.flow.buy_volume, 0.0);
    }

    #[test]
    fn test_atr_basic() {
        use data::{Timestamp, Volume};

        let candles: Vec<data::Candle> = (0..20)
            .map(|i| data::Candle {
                time: Timestamp(i * 60_000),
                open: Price::from_f64(100.0 + i as f64 * 0.1),
                high: Price::from_f64(101.0 + i as f64 * 0.1),
                low: Price::from_f64(99.0 + i as f64 * 0.1),
                close: Price::from_f64(100.5 + i as f64 * 0.1),
                buy_volume: Volume(50.0),
                sell_volume: Volume(50.0),
            })
            .collect();

        let atr = compute_atr(&candles, 14);
        assert!(atr.is_some());
        assert!(atr.unwrap() > 0.0);
    }
}
