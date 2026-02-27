//! Mathematical utilities for price rounding and panel layout distribution.
//!
//! Tick-rounding functions snap floating-point prices to the nearest valid
//! tick increment. Panel split calculation distributes vertical space between
//! the main chart and indicator panels.

/// Rounds a value to the nearest tick increment
#[must_use]
pub fn round_to_tick(value: f32, tick_size: f32) -> f32 {
    (value / tick_size).round() * tick_size
}

/// Rounds a value to the next tick boundary (floor if `down`, ceil otherwise)
#[must_use]
pub fn round_to_next_tick(value: f32, tick_size: f32, down: bool) -> f32 {
    if down {
        (value / tick_size).floor() * tick_size
    } else {
        (value / tick_size).ceil() * tick_size
    }
}

/// Estimates a reasonable tick size for a given price range magnitude
#[must_use]
pub fn guesstimate_ticks(range: f32) -> f32 {
    match range {
        r if r > 1_000_000_000.0 => 1_000_000.0,
        r if r > 100_000_000.0 => 100_000.0,
        r if r > 10_000_000.0 => 10_000.0,
        r if r > 1_000_000.0 => 1_000.0,
        r if r > 100_000.0 => 1_000.0,
        r if r > 10_000.0 => 100.0,
        r if r > 1_000.0 => 10.0,
        r if r > 100.0 => 1.0,
        r if r > 10.0 => 0.1,
        r if r > 1.0 => 0.01,
        r if r > 0.1 => 0.001,
        r if r > 0.01 => 0.0001,
        _ => 0.00001,
    }
}

/// Computes split positions for main chart and indicator panels.
///
/// Shrinks the main panel if needed when adding a new indicator panel.
/// Each indicator panel gets at least `MIN_PANEL_HEIGHT` (0.1) of the total.
#[must_use]
pub fn calc_panel_splits(
    initial_main_split: f32,
    active_indicators: usize,
    previous_indicators: Option<usize>,
) -> Vec<f32> {
    const MIN_PANEL_HEIGHT: f32 = 0.1;
    const TOTAL_HEIGHT: f32 = 1.0;

    let mut main_split = initial_main_split;

    if let Some(prev_inds) = previous_indicators
        && active_indicators > prev_inds
    {
        let min_space_needed_all_indis = active_indicators as f32 * MIN_PANEL_HEIGHT;
        let max_main_split_if_indis_get_min =
            (TOTAL_HEIGHT - min_space_needed_all_indis).max(MIN_PANEL_HEIGHT);
        if main_split > max_main_split_if_indis_get_min {
            main_split = max_main_split_if_indis_get_min;
        }
    }

    let upper_bound_for_main = if active_indicators == 0 {
        TOTAL_HEIGHT
    } else {
        (TOTAL_HEIGHT - active_indicators as f32 * MIN_PANEL_HEIGHT).max(MIN_PANEL_HEIGHT)
    };

    main_split = main_split.clamp(MIN_PANEL_HEIGHT, upper_bound_for_main);
    main_split = main_split.min(TOTAL_HEIGHT);

    let mut splits = vec![main_split];

    if active_indicators > 1 {
        let indicator_total_space = (TOTAL_HEIGHT - main_split).max(0.0);
        let per_indicator_space = indicator_total_space / active_indicators as f32;

        for i in 1..active_indicators {
            let cumulative_indicator_space = per_indicator_space * i as f32;
            let split_pos = main_split + cumulative_indicator_space;
            splits.push(split_pos.min(TOTAL_HEIGHT));
        }
    }
    splits
}
