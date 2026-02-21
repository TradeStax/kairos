//! Technical analysis functions implemented in Rust, exposed to JS.
//!
//! All functions operate on full arrays (not bar-by-bar). They take
//! `Vec<f64>` inputs and return `Vec<f64>` (or JS objects for
//! multi-output indicators like MACD, Stochastic, Bollinger Bands).

// Sliding-window algorithms intentionally use indexed loops for clarity
// when the index is needed for both window-bound computation and output indexing.
#![allow(clippy::needless_range_loop)]
//!
//! NaN is used for points where there is insufficient data.

use crate::error::ScriptError;
use rquickjs::{Ctx, Function, Object};
use std::collections::{BTreeMap, HashMap};

// ── Helpers ────────────────────────────────────────────────────────

/// Sliding-window SMA over `source`. Returns a vec of the same length
/// with NaN for the first `period - 1` elements.
fn compute_sma(source: &[f64], period: usize) -> Vec<f64> {
    let len = source.len();
    if len < period || period == 0 {
        return vec![f64::NAN; len];
    }
    let mut out = vec![f64::NAN; len];
    let mut sum: f64 = source[..period].iter().sum();
    out[period - 1] = sum / period as f64;
    for i in period..len {
        sum += source[i] - source[i - period];
        out[i] = sum / period as f64;
    }
    out
}

/// EMA seeded with SMA, multiplier = 2 / (period + 1).
fn compute_ema(source: &[f64], period: usize) -> Vec<f64> {
    let len = source.len();
    if len < period || period == 0 {
        return vec![f64::NAN; len];
    }
    let mult = 2.0 / (period + 1) as f64;
    let mut out = vec![f64::NAN; len];
    let sma: f64 = source[..period].iter().sum::<f64>() / period as f64;
    out[period - 1] = sma;
    let mut prev = sma;
    for i in period..len {
        let v = source[i] * mult + prev * (1.0 - mult);
        out[i] = v;
        prev = v;
    }
    out
}

/// Wilder's smoothed moving average (RMA).
/// Seed with SMA, then rma = (prev * (period-1) + val) / period.
fn compute_rma(source: &[f64], period: usize) -> Vec<f64> {
    let len = source.len();
    if len < period || period == 0 {
        return vec![f64::NAN; len];
    }
    let mut out = vec![f64::NAN; len];
    let sma: f64 = source[..period].iter().sum::<f64>() / period as f64;
    out[period - 1] = sma;
    let mut prev = sma;
    for i in period..len {
        let v = (prev * (period as f64 - 1.0) + source[i]) / period as f64;
        out[i] = v;
        prev = v;
    }
    out
}

/// Build a volume profile from bar data, distributing each bar's
/// volume evenly across tick-size price levels from low to high.
/// Returns sorted (price -> (buy_vol, sell_vol)) map.
fn build_volume_profile(
    high: &[f64],
    low: &[f64],
    buy_volume: &[f64],
    sell_volume: &[f64],
    tick_size: f64,
) -> BTreeMap<i64, (f64, f64)> {
    let mut levels: BTreeMap<i64, (f64, f64)> = BTreeMap::new();
    let len = high.len();
    for i in 0..len {
        let num_levels =
            ((high[i] - low[i]) / tick_size).floor() as usize + 1;
        if num_levels == 0 {
            continue;
        }
        let buy_per = buy_volume[i] / num_levels as f64;
        let sell_per = sell_volume[i] / num_levels as f64;
        for l in 0..num_levels {
            let price = low[i] + l as f64 * tick_size;
            let key =
                (price / tick_size).round() as i64;
            let entry = levels.entry(key).or_insert((0.0, 0.0));
            entry.0 += buy_per;
            entry.1 += sell_per;
        }
    }
    levels
}

/// Find POC index (max total volume) and value area indices
/// that capture `pct` of total volume, expanding from POC.
fn find_value_area(
    volumes: &[(f64, f64)],
    pct: f64,
) -> Option<(usize, usize, usize)> {
    if volumes.is_empty() {
        return None;
    }
    let totals: Vec<f64> =
        volumes.iter().map(|(b, s)| b + s).collect();
    let total_vol: f64 = totals.iter().sum();
    if total_vol <= 0.0 {
        return None;
    }
    // Find POC
    let poc_idx = totals
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .unwrap();
    // Expand from POC
    let target = total_vol * pct;
    let mut accumulated = totals[poc_idx];
    let mut lo = poc_idx;
    let mut hi = poc_idx;
    while accumulated < target && (lo > 0 || hi < totals.len() - 1) {
        let up_vol = if hi < totals.len() - 1 {
            totals[hi + 1]
        } else {
            -1.0
        };
        let dn_vol = if lo > 0 { totals[lo - 1] } else { -1.0 };
        if up_vol >= dn_vol {
            hi += 1;
            accumulated += totals[hi];
        } else {
            lo -= 1;
            accumulated += totals[lo];
        }
    }
    Some((poc_idx, lo, hi))
}

// ── JS-returning orderflow helpers ────────────────────────────────

fn ta_build_profile<'js>(
    ctx: Ctx<'js>,
    high: Vec<f64>,
    low: Vec<f64>,
    buy_volume: Vec<f64>,
    sell_volume: Vec<f64>,
    tick_size: f64,
) -> rquickjs::Result<Object<'js>> {
    let len = high.len();
    let result = Object::new(ctx.clone())?;
    if len == 0
        || low.len() != len
        || buy_volume.len() != len
        || sell_volume.len() != len
        || tick_size <= 0.0
    {
        result.set("prices", Vec::<f64>::new())?;
        result.set("buyVolumes", Vec::<f64>::new())?;
        result.set("sellVolumes", Vec::<f64>::new())?;
        result.set("pocIndex", -1i32)?;
        result.set(
            "valueArea",
            rquickjs::Value::new_null(ctx.clone()),
        )?;
        return Ok(result);
    }
    let levels = build_volume_profile(
        &high,
        &low,
        &buy_volume,
        &sell_volume,
        tick_size,
    );
    let mut prices = Vec::with_capacity(levels.len());
    let mut buys = Vec::with_capacity(levels.len());
    let mut sells = Vec::with_capacity(levels.len());
    for (&key, &(b, s)) in &levels {
        prices.push(key as f64 * tick_size);
        buys.push(b);
        sells.push(s);
    }
    let volumes: Vec<(f64, f64)> = buys
        .iter()
        .zip(sells.iter())
        .map(|(&b, &s)| (b, s))
        .collect();
    let va = find_value_area(&volumes, 0.70);
    result.set("prices", prices)?;
    result.set("buyVolumes", buys)?;
    result.set("sellVolumes", sells)?;
    match va {
        Some((poc, lo, hi)) => {
            result.set("pocIndex", poc as i32)?;
            let va_obj = Object::new(ctx.clone())?;
            va_obj.set("vahIndex", hi as i32)?;
            va_obj.set("valIndex", lo as i32)?;
            result.set("valueArea", va_obj)?;
        }
        None => {
            result.set("pocIndex", -1i32)?;
            result.set(
                "valueArea",
                rquickjs::Value::new_null(ctx.clone()),
            )?;
        }
    }
    Ok(result)
}

fn ta_build_footprint<'js>(
    ctx: Ctx<'js>,
    ohlc_val: rquickjs::Value<'js>,
    trades_val: rquickjs::Value<'js>,
    tick_size: f64,
) -> rquickjs::Result<Object<'js>> {
    let result = Object::new(ctx.clone())?;
    let ohlc = ohlc_val.into_object().ok_or_else(|| {
        rquickjs::Error::new_from_js("value", "object")
    })?;
    let time_arr: Vec<f64> = ohlc.get("time")?;
    let open_arr: Vec<f64> = ohlc.get("open")?;
    let high_arr: Vec<f64> = ohlc.get("high")?;
    let low_arr: Vec<f64> = ohlc.get("low")?;
    let close_arr: Vec<f64> = ohlc.get("close")?;
    let len = time_arr.len();
    let trades_js = trades_val.into_array().ok_or_else(|| {
        rquickjs::Error::new_from_js("value", "array")
    })?;
    struct RawTrade {
        time: f64,
        price: f64,
        quantity: f64,
        is_buy: bool,
    }
    let trade_count = trades_js.len();
    let mut trades = Vec::with_capacity(trade_count);
    for idx in 0..trade_count {
        let val: rquickjs::Value = trades_js.get(idx)?;
        if let Some(obj) = val.as_object() {
            let t: f64 = obj.get("time")?;
            let p: f64 = obj.get("price")?;
            let q: f64 = obj.get("quantity")?;
            let b: bool = obj.get("isBuy")?;
            trades.push(RawTrade {
                time: t,
                price: p,
                quantity: q,
                is_buy: b,
            });
        }
    }
    trades.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let candles_js = rquickjs::Array::new(ctx.clone())?;
    let mut t_idx = 0usize;
    for i in 0..len {
        let candle_obj = Object::new(ctx.clone())?;
        candle_obj.set("x", time_arr[i])?;
        candle_obj.set("open", open_arr[i])?;
        candle_obj.set("high", high_arr[i])?;
        candle_obj.set("low", low_arr[i])?;
        candle_obj.set("close", close_arr[i])?;
        let next_time = if i + 1 < len {
            time_arr[i + 1]
        } else {
            f64::INFINITY
        };
        let mut candle_levels: BTreeMap<i64, (f64, f64)> =
            BTreeMap::new();
        while t_idx < trades.len()
            && trades[t_idx].time < next_time
        {
            if trades[t_idx].time >= time_arr[i] {
                let key = (trades[t_idx].price / tick_size)
                    .round() as i64;
                let entry = candle_levels
                    .entry(key)
                    .or_insert((0.0, 0.0));
                if trades[t_idx].is_buy {
                    entry.0 += trades[t_idx].quantity;
                } else {
                    entry.1 += trades[t_idx].quantity;
                }
            }
            t_idx += 1;
        }
        let levels_js =
            rquickjs::Array::new(ctx.clone())?;
        let mut poc_idx: i32 = -1;
        let mut poc_vol = 0.0f64;
        for (lvl_idx, (&key, &(b, s))) in
            candle_levels.iter().enumerate()
        {
            let lvl_obj = Object::new(ctx.clone())?;
            lvl_obj
                .set("price", key as f64 * tick_size)?;
            lvl_obj.set("buy", b)?;
            lvl_obj.set("sell", s)?;
            levels_js.set(lvl_idx, lvl_obj)?;
            if b + s > poc_vol {
                poc_vol = b + s;
                poc_idx = lvl_idx as i32;
            }
        }
        candle_obj.set("levels", levels_js)?;
        candle_obj.set("pocIndex", poc_idx)?;
        candles_js.set(i, candle_obj)?;
    }
    result.set("candles", candles_js)?;
    Ok(result)
}

fn ta_value_area<'js>(
    ctx: Ctx<'js>,
    high: Vec<f64>,
    low: Vec<f64>,
    volume: Vec<f64>,
    tick_size: f64,
    percentage: f64,
) -> rquickjs::Result<Object<'js>> {
    let result = Object::new(ctx.clone())?;
    let len = high.len();
    if len == 0
        || low.len() != len
        || volume.len() != len
        || tick_size <= 0.0
    {
        result.set(
            "vah",
            rquickjs::Value::new_null(ctx.clone()),
        )?;
        result.set(
            "val",
            rquickjs::Value::new_null(ctx.clone()),
        )?;
        return Ok(result);
    }
    let half: Vec<f64> =
        volume.iter().map(|v| v / 2.0).collect();
    let levels = build_volume_profile(
        &high, &low, &half, &half, tick_size,
    );
    let prices: Vec<f64> = levels
        .keys()
        .map(|&k| k as f64 * tick_size)
        .collect();
    let volumes: Vec<(f64, f64)> =
        levels.values().copied().collect();
    match find_value_area(&volumes, percentage) {
        Some((_, lo, hi)) => {
            result.set("vah", prices[hi])?;
            result.set("val", prices[lo])?;
        }
        None => {
            result.set(
                "vah",
                rquickjs::Value::new_null(ctx.clone()),
            )?;
            result.set(
                "val",
                rquickjs::Value::new_null(ctx.clone()),
            )?;
        }
    }
    Ok(result)
}

// ── Install TA object ──────────────────────────────────────────────

pub fn install_ta(ctx: &Ctx<'_>) -> Result<(), ScriptError> {
    let globals = ctx.globals();
    let ta = Object::new(ctx.clone())?;

    // ── Moving Averages ────────────────────────────────────────

    ta.set(
        "sma",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>, period: usize| -> Vec<f64> {
                compute_sma(&source, period)
            },
        ),
    )?;

    ta.set(
        "ema",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>, period: usize| -> Vec<f64> {
                compute_ema(&source, period)
            },
        ),
    )?;

    ta.set(
        "wma",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>, period: usize| -> Vec<f64> {
                let len = source.len();
                if len < period || period == 0 {
                    return vec![f64::NAN; len];
                }
                let denom =
                    (period * (period + 1)) as f64 / 2.0;
                let mut out = vec![f64::NAN; len];
                for i in (period - 1)..len {
                    let start = i + 1 - period;
                    let mut sum = 0.0;
                    for (w, j) in (start..=i).enumerate() {
                        sum += source[j] * (w + 1) as f64;
                    }
                    out[i] = sum / denom;
                }
                out
            },
        ),
    )?;

    ta.set(
        "vwma",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>,
             volume: Vec<f64>,
             period: usize|
             -> Vec<f64> {
                let len = source.len();
                if len < period
                    || period == 0
                    || volume.len() != len
                {
                    return vec![f64::NAN; len];
                }
                let mut out = vec![f64::NAN; len];
                let mut sum_sv: f64 = 0.0;
                let mut sum_v: f64 = 0.0;
                for j in 0..period {
                    sum_sv += source[j] * volume[j];
                    sum_v += volume[j];
                }
                out[period - 1] = if sum_v != 0.0 {
                    sum_sv / sum_v
                } else {
                    f64::NAN
                };
                for i in period..len {
                    sum_sv += source[i] * volume[i]
                        - source[i - period] * volume[i - period];
                    sum_v +=
                        volume[i] - volume[i - period];
                    out[i] = if sum_v != 0.0 {
                        sum_sv / sum_v
                    } else {
                        f64::NAN
                    };
                }
                out
            },
        ),
    )?;

    ta.set(
        "rma",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>, period: usize| -> Vec<f64> {
                compute_rma(&source, period)
            },
        ),
    )?;

    // ── Oscillators ────────────────────────────────────────────

    ta.set(
        "rsi",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>, period: usize| -> Vec<f64> {
                let len = source.len();
                if len < period + 1 || period == 0 {
                    return vec![f64::NAN; len];
                }
                let mut out = vec![f64::NAN; len];

                let mut avg_gain: f64 = 0.0;
                let mut avg_loss: f64 = 0.0;
                for i in 1..=period {
                    let change = source[i] - source[i - 1];
                    if change > 0.0 {
                        avg_gain += change;
                    } else {
                        avg_loss -= change;
                    }
                }
                avg_gain /= period as f64;
                avg_loss /= period as f64;

                out[period] = if avg_loss == 0.0 {
                    100.0
                } else {
                    100.0
                        - 100.0 / (1.0 + avg_gain / avg_loss)
                };

                for i in (period + 1)..len {
                    let change = source[i] - source[i - 1];
                    let (gain, loss) = if change > 0.0 {
                        (change, 0.0)
                    } else {
                        (0.0, -change)
                    };
                    avg_gain = (avg_gain * (period as f64 - 1.0)
                        + gain)
                        / period as f64;
                    avg_loss = (avg_loss * (period as f64 - 1.0)
                        + loss)
                        / period as f64;
                    out[i] = if avg_loss == 0.0 {
                        100.0
                    } else {
                        100.0
                            - 100.0
                                / (1.0 + avg_gain / avg_loss)
                    };
                }
                out
            },
        ),
    )?;

    // stoch returns {k, d}
    ta.set(
        "stoch",
        Function::new(
            ctx.clone(),
            |high: Vec<f64>,
             low: Vec<f64>,
             close: Vec<f64>,
             period_k: usize,
             period_d: usize,
             slowing: usize|
             -> HashMap<String, Vec<f64>> {
                let len = close.len();
                if len < period_k
                    || high.len() != len
                    || low.len() != len
                    || period_k == 0
                {
                    let nan_vec = vec![f64::NAN; len];
                    let mut m = HashMap::new();
                    m.insert("k".to_string(), nan_vec.clone());
                    m.insert("d".to_string(), nan_vec);
                    return m;
                }

                // Raw %K
                let mut raw_k = vec![f64::NAN; len];
                for i in (period_k - 1)..len {
                    let start = i + 1 - period_k;
                    let mut lo = f64::MAX;
                    let mut hi = f64::MIN;
                    for j in start..=i {
                        if low[j] < lo {
                            lo = low[j];
                        }
                        if high[j] > hi {
                            hi = high[j];
                        }
                    }
                    let range = hi - lo;
                    raw_k[i] = if range > 0.0 {
                        100.0 * (close[i] - lo) / range
                    } else {
                        50.0
                    };
                }

                // Slow %K = SMA(raw_k, slowing)
                let slow = slowing.max(1);
                let k_vals = if slow > 1 {
                    sma_nan(&raw_k, slow)
                } else {
                    raw_k
                };

                // %D = SMA(%K, period_d)
                let d_period = period_d.max(1);
                let d_vals = if d_period > 1 {
                    sma_nan(&k_vals, d_period)
                } else {
                    k_vals.clone()
                };

                let mut m = HashMap::new();
                m.insert("k".to_string(), k_vals);
                m.insert("d".to_string(), d_vals);
                m
            },
        ),
    )?;

    // macd returns {macd, signal, histogram}
    ta.set(
        "macd",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>,
             fast: usize,
             slow: usize,
             signal_period: usize|
             -> HashMap<String, Vec<f64>> {
                let len = source.len();

                let fast_ema = compute_ema(&source, fast);
                let slow_ema = compute_ema(&source, slow);

                let mut macd_line = vec![f64::NAN; len];
                for i in 0..len {
                    if fast_ema[i].is_nan()
                        || slow_ema[i].is_nan()
                    {
                        continue;
                    }
                    macd_line[i] = fast_ema[i] - slow_ema[i];
                }

                // Signal = EMA of non-NaN MACD values
                let signal_line =
                    ema_of_nan_series(&macd_line, signal_period);

                let mut hist = vec![f64::NAN; len];
                for i in 0..len {
                    if !macd_line[i].is_nan()
                        && !signal_line[i].is_nan()
                    {
                        hist[i] = macd_line[i] - signal_line[i];
                    }
                }

                let mut m = HashMap::new();
                m.insert("macd".to_string(), macd_line);
                m.insert("signal".to_string(), signal_line);
                m.insert("histogram".to_string(), hist);
                m
            },
        ),
    )?;

    // ── Volatility ─────────────────────────────────────────────

    ta.set(
        "atr",
        Function::new(
            ctx.clone(),
            |high: Vec<f64>,
             low: Vec<f64>,
             close: Vec<f64>,
             period: usize|
             -> Vec<f64> {
                let len = close.len();
                if len < 2
                    || high.len() != len
                    || low.len() != len
                    || period == 0
                {
                    return vec![f64::NAN; len];
                }
                // True Range series (len - 1 values, index 0
                // corresponds to candle index 1)
                let mut tr = vec![0.0f64; len - 1];
                for i in 0..(len - 1) {
                    let h = high[i + 1];
                    let l = low[i + 1];
                    let pc = close[i];
                    tr[i] = (h - l)
                        .max((h - pc).abs())
                        .max((l - pc).abs());
                }

                let mut out = vec![f64::NAN; len];
                if tr.len() < period {
                    return out;
                }
                // Seed ATR with SMA of first `period` TRs
                let mut atr: f64 = tr[..period].iter().sum::<f64>()
                    / period as f64;
                // First ATR at candle index `period`
                out[period] = atr;
                // Wilder's smoothing
                for i in period..tr.len() {
                    atr = (atr * (period as f64 - 1.0) + tr[i])
                        / period as f64;
                    out[i + 1] = atr;
                }
                out
            },
        ),
    )?;

    // bb returns {upper, middle, lower}
    ta.set(
        "bb",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>,
             period: usize,
             mult: f64|
             -> HashMap<String, Vec<f64>> {
                let len = source.len();
                let mut upper = vec![f64::NAN; len];
                let mut middle = vec![f64::NAN; len];
                let mut lower = vec![f64::NAN; len];

                if len >= period && period > 0 {
                    for i in (period - 1)..len {
                        let start = i + 1 - period;
                        let window = &source[start..=i];
                        let mean = window.iter().sum::<f64>()
                            / period as f64;
                        let var = window
                            .iter()
                            .map(|v| (v - mean).powi(2))
                            .sum::<f64>()
                            / period as f64;
                        let sd = var.sqrt();
                        middle[i] = mean;
                        upper[i] = mean + mult * sd;
                        lower[i] = mean - mult * sd;
                    }
                }

                let mut m = HashMap::new();
                m.insert("upper".to_string(), upper);
                m.insert("middle".to_string(), middle);
                m.insert("lower".to_string(), lower);
                m
            },
        ),
    )?;

    // ── Volume ─────────────────────────────────────────────────

    ta.set(
        "obv",
        Function::new(
            ctx.clone(),
            |close: Vec<f64>, volume: Vec<f64>| -> Vec<f64> {
                let len = close.len();
                if len == 0 || volume.len() != len {
                    return vec![];
                }
                let mut out = Vec::with_capacity(len);
                out.push(0.0);
                let mut obv = 0.0f64;
                for i in 1..len {
                    if close[i] > close[i - 1] {
                        obv += volume[i];
                    } else if close[i] < close[i - 1] {
                        obv -= volume[i];
                    }
                    out.push(obv);
                }
                out
            },
        ),
    )?;

    ta.set(
        "cvd",
        Function::new(
            ctx.clone(),
            |buy_volume: Vec<f64>,
             sell_volume: Vec<f64>|
             -> Vec<f64> {
                let len = buy_volume.len();
                if len == 0 || sell_volume.len() != len {
                    return vec![];
                }
                let mut out = Vec::with_capacity(len);
                let mut cum = 0.0f64;
                for i in 0..len {
                    cum += buy_volume[i] - sell_volume[i];
                    out.push(cum);
                }
                out
            },
        ),
    )?;

    ta.set(
        "vwap",
        Function::new(
            ctx.clone(),
            |high: Vec<f64>,
             low: Vec<f64>,
             close: Vec<f64>,
             volume: Vec<f64>|
             -> Vec<f64> {
                let len = close.len();
                if len == 0
                    || high.len() != len
                    || low.len() != len
                    || volume.len() != len
                {
                    return vec![];
                }
                let mut out = Vec::with_capacity(len);
                let mut cum_tp_vol = 0.0f64;
                let mut cum_vol = 0.0f64;
                for i in 0..len {
                    let tp =
                        (high[i] + low[i] + close[i]) / 3.0;
                    cum_tp_vol += tp * volume[i];
                    cum_vol += volume[i];
                    out.push(if cum_vol > 0.0 {
                        cum_tp_vol / cum_vol
                    } else {
                        tp
                    });
                }
                out
            },
        ),
    )?;

    // ── Utilities ──────────────────────────────────────────────

    ta.set(
        "crossover",
        Function::new(
            ctx.clone(),
            |a: Vec<f64>, b: Vec<f64>| -> Vec<bool> {
                let len = a.len().min(b.len());
                let mut out = vec![false; len];
                for i in 1..len {
                    out[i] =
                        a[i] > b[i] && a[i - 1] <= b[i - 1];
                }
                out
            },
        ),
    )?;

    ta.set(
        "crossunder",
        Function::new(
            ctx.clone(),
            |a: Vec<f64>, b: Vec<f64>| -> Vec<bool> {
                let len = a.len().min(b.len());
                let mut out = vec![false; len];
                for i in 1..len {
                    out[i] =
                        a[i] < b[i] && a[i - 1] >= b[i - 1];
                }
                out
            },
        ),
    )?;

    ta.set(
        "highest",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>, period: usize| -> Vec<f64> {
                let len = source.len();
                if period == 0 {
                    return vec![f64::NAN; len];
                }
                let mut out = vec![f64::NAN; len];
                for i in (period - 1)..len {
                    let start = i + 1 - period;
                    let mut hi = f64::NEG_INFINITY;
                    for j in start..=i {
                        if source[j] > hi {
                            hi = source[j];
                        }
                    }
                    out[i] = hi;
                }
                out
            },
        ),
    )?;

    ta.set(
        "lowest",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>, period: usize| -> Vec<f64> {
                let len = source.len();
                if period == 0 {
                    return vec![f64::NAN; len];
                }
                let mut out = vec![f64::NAN; len];
                for i in (period - 1)..len {
                    let start = i + 1 - period;
                    let mut lo = f64::INFINITY;
                    for j in start..=i {
                        if source[j] < lo {
                            lo = source[j];
                        }
                    }
                    out[i] = lo;
                }
                out
            },
        ),
    )?;

    ta.set(
        "change",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>, length: usize| -> Vec<f64> {
                let len = source.len();
                let mut out = vec![f64::NAN; len];
                for i in length..len {
                    out[i] = source[i] - source[i - length];
                }
                out
            },
        ),
    )?;

    ta.set(
        "roc",
        Function::new(
            ctx.clone(),
            |source: Vec<f64>, length: usize| -> Vec<f64> {
                let len = source.len();
                let mut out = vec![f64::NAN; len];
                for i in length..len {
                    let prev = source[i - length];
                    out[i] = if prev != 0.0 {
                        (source[i] - prev) / prev * 100.0
                    } else {
                        f64::NAN
                    };
                }
                out
            },
        ),
    )?;

    // ── Orderflow ────────────────────────────────────────────

    // cvdReset: CVD with periodic reset (daily/weekly)
    ta.set(
        "cvdReset",
        Function::new(
            ctx.clone(),
            |buy_volume: Vec<f64>,
             sell_volume: Vec<f64>,
             time: Vec<f64>,
             reset_period: String|
             -> Vec<f64> {
                let len = buy_volume.len();
                if len == 0
                    || sell_volume.len() != len
                    || time.len() != len
                {
                    return vec![];
                }
                let divisor: f64 = match reset_period.as_str() {
                    "weekly" => 604_800_000.0,
                    _ => 86_400_000.0, // daily default
                };
                let mut out = Vec::with_capacity(len);
                let mut cum = 0.0f64;
                let mut prev_bucket =
                    (time[0] / divisor).floor() as i64;
                for i in 0..len {
                    let bucket =
                        (time[i] / divisor).floor() as i64;
                    if bucket != prev_bucket {
                        cum = 0.0;
                        prev_bucket = bucket;
                    }
                    cum += buy_volume[i] - sell_volume[i];
                    out.push(cum);
                }
                out
            },
        ),
    )?;

    // vwapBands: VWAP with standard deviation bands
    ta.set(
        "vwapBands",
        Function::new(
            ctx.clone(),
            |high: Vec<f64>,
             low: Vec<f64>,
             close: Vec<f64>,
             volume: Vec<f64>,
             multiplier: f64|
             -> HashMap<String, Vec<f64>> {
                let len = close.len();
                if len == 0
                    || high.len() != len
                    || low.len() != len
                    || volume.len() != len
                {
                    let mut m = HashMap::new();
                    m.insert("vwap".into(), vec![]);
                    m.insert("upper".into(), vec![]);
                    m.insert("lower".into(), vec![]);
                    return m;
                }
                let mut vwap_out = Vec::with_capacity(len);
                let mut upper_out = Vec::with_capacity(len);
                let mut lower_out = Vec::with_capacity(len);
                let mut cum_tp_vol = 0.0f64;
                let mut cum_tp2_vol = 0.0f64;
                let mut cum_vol = 0.0f64;
                for i in 0..len {
                    let tp =
                        (high[i] + low[i] + close[i]) / 3.0;
                    cum_tp_vol += tp * volume[i];
                    cum_tp2_vol += tp * tp * volume[i];
                    cum_vol += volume[i];
                    if cum_vol > 0.0 {
                        let vwap = cum_tp_vol / cum_vol;
                        let variance =
                            cum_tp2_vol / cum_vol - vwap * vwap;
                        let stdev =
                            variance.max(0.0).sqrt();
                        vwap_out.push(vwap);
                        upper_out
                            .push(vwap + multiplier * stdev);
                        lower_out
                            .push(vwap - multiplier * stdev);
                    } else {
                        vwap_out.push(tp);
                        upper_out.push(tp);
                        lower_out.push(tp);
                    }
                }
                let mut m = HashMap::new();
                m.insert("vwap".into(), vwap_out);
                m.insert("upper".into(), upper_out);
                m.insert("lower".into(), lower_out);
                m
            },
        ),
    )?;

    // buildProfile: volume profile across all bars
    ta.set(
        "buildProfile",
        Function::new(ctx.clone(), ta_build_profile),
    )?;

    // buildFootprint: per-candle footprint from trade data
    // Accepts: buildFootprint({time,open,high,low,close},
    //          trades, tickSize)
    ta.set(
        "buildFootprint",
        Function::new(ctx.clone(), ta_build_footprint),
    )?;

    // rollingPoc: rolling window POC price
    ta.set(
        "rollingPoc",
        Function::new(
            ctx.clone(),
            |high: Vec<f64>,
             low: Vec<f64>,
             volume: Vec<f64>,
             tick_size: f64,
             lookback: usize|
             -> Vec<f64> {
                let len = high.len();
                if len == 0
                    || low.len() != len
                    || volume.len() != len
                    || tick_size <= 0.0
                    || lookback == 0
                {
                    return vec![f64::NAN; len];
                }
                let mut out = vec![f64::NAN; len];
                // Use equal buy/sell split (total
                // volume only)
                let half: Vec<f64> = volume
                    .iter()
                    .map(|v| v / 2.0)
                    .collect();
                for i in (lookback - 1)..len {
                    let start = i + 1 - lookback;
                    let levels = build_volume_profile(
                        &high[start..=i],
                        &low[start..=i],
                        &half[start..=i],
                        &half[start..=i],
                        tick_size,
                    );
                    if let Some((&poc_key, _)) = levels
                        .iter()
                        .max_by(|(_, (a0, a1)), (_, (b0, b1))| {
                            (a0 + a1)
                                .partial_cmp(&(b0 + b1))
                                .unwrap_or(
                                    std::cmp::Ordering::Equal,
                                )
                        })
                    {
                        out[i] =
                            poc_key as f64 * tick_size;
                    }
                }
                out
            },
        ),
    )?;

    // valueArea: full-data value area (VAH/VAL)
    ta.set(
        "valueArea",
        Function::new(ctx.clone(), ta_value_area),
    )?;

    globals.set("ta", ta)?;
    Ok(())
}

// ── Stubs for declaration pass ─────────────────────────────────────

/// Install ta.* stubs that return empty arrays, for the declaration
/// pass where no candle data is available.
pub fn install_ta_stubs(
    ctx: &Ctx<'_>,
) -> Result<(), ScriptError> {
    let globals = ctx.globals();
    let ta = Object::new(ctx.clone())?;

    // Helper: register a function that returns an empty Vec<f64>
    macro_rules! stub_arr {
        ($name:expr, $($arg:ident : $t:ty),*) => {
            ta.set(
                $name,
                Function::new(
                    ctx.clone(),
                    |$($arg: $t),*| -> Vec<f64> { vec![] },
                ),
            )?;
        };
    }

    // Moving averages
    stub_arr!("sma", _s: Vec<f64>, _p: usize);
    stub_arr!("ema", _s: Vec<f64>, _p: usize);
    stub_arr!("wma", _s: Vec<f64>, _p: usize);
    stub_arr!("vwma", _s: Vec<f64>, _v: Vec<f64>, _p: usize);
    stub_arr!("rma", _s: Vec<f64>, _p: usize);

    // Oscillators
    stub_arr!("rsi", _s: Vec<f64>, _p: usize);

    // stoch stub -> returns {k: [], d: []}
    ta.set(
        "stoch",
        Function::new(
            ctx.clone(),
            |_h: Vec<f64>,
             _l: Vec<f64>,
             _c: Vec<f64>,
             _pk: usize,
             _pd: usize,
             _sl: usize|
             -> HashMap<String, Vec<f64>> {
                let mut m = HashMap::new();
                m.insert("k".to_string(), vec![]);
                m.insert("d".to_string(), vec![]);
                m
            },
        ),
    )?;

    // macd stub -> returns {macd: [], signal: [], histogram: []}
    ta.set(
        "macd",
        Function::new(
            ctx.clone(),
            |_s: Vec<f64>,
             _f: usize,
             _sl: usize,
             _sg: usize|
             -> HashMap<String, Vec<f64>> {
                let mut m = HashMap::new();
                m.insert("macd".to_string(), vec![]);
                m.insert("signal".to_string(), vec![]);
                m.insert("histogram".to_string(), vec![]);
                m
            },
        ),
    )?;

    // Volatility
    stub_arr!(
        "atr", _h: Vec<f64>, _l: Vec<f64>,
        _c: Vec<f64>, _p: usize
    );

    // bb stub -> returns {upper: [], middle: [], lower: []}
    ta.set(
        "bb",
        Function::new(
            ctx.clone(),
            |_s: Vec<f64>,
             _p: usize,
             _m: f64|
             -> HashMap<String, Vec<f64>> {
                let mut m = HashMap::new();
                m.insert("upper".to_string(), vec![]);
                m.insert("middle".to_string(), vec![]);
                m.insert("lower".to_string(), vec![]);
                m
            },
        ),
    )?;

    // Volume
    stub_arr!("obv", _c: Vec<f64>, _v: Vec<f64>);
    stub_arr!("cvd", _b: Vec<f64>, _s: Vec<f64>);
    stub_arr!(
        "vwap", _h: Vec<f64>, _l: Vec<f64>,
        _c: Vec<f64>, _v: Vec<f64>
    );

    // Utilities
    ta.set(
        "crossover",
        Function::new(
            ctx.clone(),
            |_a: Vec<f64>, _b: Vec<f64>| -> Vec<bool> {
                vec![]
            },
        ),
    )?;
    ta.set(
        "crossunder",
        Function::new(
            ctx.clone(),
            |_a: Vec<f64>, _b: Vec<f64>| -> Vec<bool> {
                vec![]
            },
        ),
    )?;
    stub_arr!("highest", _s: Vec<f64>, _p: usize);
    stub_arr!("lowest", _s: Vec<f64>, _p: usize);
    stub_arr!("change", _s: Vec<f64>, _l: usize);
    stub_arr!("roc", _s: Vec<f64>, _l: usize);

    // Orderflow stubs
    stub_arr!(
        "cvdReset", _b: Vec<f64>, _s: Vec<f64>,
        _t: Vec<f64>, _r: String
    );

    // vwapBands stub -> {vwap: [], upper: [], lower: []}
    ta.set(
        "vwapBands",
        Function::new(
            ctx.clone(),
            |_h: Vec<f64>,
             _l: Vec<f64>,
             _c: Vec<f64>,
             _v: Vec<f64>,
             _m: f64|
             -> HashMap<String, Vec<f64>> {
                let mut m = HashMap::new();
                m.insert("vwap".to_string(), vec![]);
                m.insert("upper".to_string(), vec![]);
                m.insert("lower".to_string(), vec![]);
                m
            },
        ),
    )?;

    // buildProfile stub -> empty object
    ta.set(
        "buildProfile",
        Function::new(
            ctx.clone(),
            |_h: Vec<f64>,
             _l: Vec<f64>,
             _b: Vec<f64>,
             _s: Vec<f64>,
             _t: f64|
             -> HashMap<String, Vec<f64>> {
                HashMap::new()
            },
        ),
    )?;

    // buildFootprint stub -> empty object
    ta.set(
        "buildFootprint",
        Function::new(
            ctx.clone(),
            |_ohlc: rquickjs::Value<'_>,
             _trades: rquickjs::Value<'_>,
             _ts: f64|
             -> HashMap<String, Vec<f64>> {
                HashMap::new()
            },
        ),
    )?;

    // rollingPoc stub
    stub_arr!(
        "rollingPoc", _h: Vec<f64>, _l: Vec<f64>,
        _v: Vec<f64>, _t: f64, _lb: usize
    );

    // valueArea stub -> empty object
    ta.set(
        "valueArea",
        Function::new(
            ctx.clone(),
            |_h: Vec<f64>,
             _l: Vec<f64>,
             _v: Vec<f64>,
             _t: f64,
             _p: f64|
             -> HashMap<String, Vec<f64>> {
                HashMap::new()
            },
        ),
    )?;

    globals.set("ta", ta)?;
    Ok(())
}

// ── Internal helpers for multi-output indicators ───────────────────

/// SMA that preserves NaN positions. For a window to produce a value,
/// all inputs in the window must be non-NaN.
fn sma_nan(source: &[f64], period: usize) -> Vec<f64> {
    let len = source.len();
    if period == 0 {
        return vec![f64::NAN; len];
    }
    let mut out = vec![f64::NAN; len];
    for i in 0..len {
        if source[i].is_nan() {
            continue;
        }
        if i + 1 < period {
            continue;
        }
        let start = i + 1 - period;
        let mut sum = 0.0;
        let mut valid = true;
        for j in start..=i {
            if source[j].is_nan() {
                valid = false;
                break;
            }
            sum += source[j];
        }
        if valid {
            out[i] = sum / period as f64;
        }
    }
    out
}

/// Compute EMA over a series that may start with NaN values.
/// Finds the first run of `period` consecutive non-NaN values,
/// seeds with their SMA, then applies EMA forward (skipping any
/// remaining NaN values).
fn ema_of_nan_series(
    source: &[f64],
    period: usize,
) -> Vec<f64> {
    let len = source.len();
    if period == 0 {
        return vec![f64::NAN; len];
    }
    let mut out = vec![f64::NAN; len];

    // Find first index where `period` consecutive non-NaN exist
    let mut consecutive = 0usize;
    let mut seed_end = None;
    for i in 0..len {
        if source[i].is_nan() {
            consecutive = 0;
        } else {
            consecutive += 1;
            if consecutive >= period {
                seed_end = Some(i);
                break;
            }
        }
    }
    let seed_end = match seed_end {
        Some(e) => e,
        None => return out,
    };
    let seed_start = seed_end + 1 - period;
    let sma: f64 = source[seed_start..=seed_end].iter().sum::<f64>()
        / period as f64;
    out[seed_end] = sma;

    let mult = 2.0 / (period + 1) as f64;
    let mut prev = sma;
    for i in (seed_end + 1)..len {
        if source[i].is_nan() {
            continue;
        }
        let v = source[i] * mult + prev * (1.0 - mult);
        out[i] = v;
        prev = v;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_sma() {
        let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let result = compute_sma(&data, 3);
        assert_eq!(result.len(), 5);
        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert!((result[2] - 20.0).abs() < 1e-10);
        assert!((result[3] - 30.0).abs() < 1e-10);
        assert!((result[4] - 40.0).abs() < 1e-10);
    }

    #[test]
    fn test_compute_ema() {
        let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let result = compute_ema(&data, 3);
        assert_eq!(result.len(), 5);
        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        // Seed = SMA(10,20,30) = 20.0
        assert!((result[2] - 20.0).abs() < 1e-10);
        // mult = 2/4 = 0.5; EMA = 40*0.5 + 20*0.5 = 30
        assert!((result[3] - 30.0).abs() < 1e-10);
        // EMA = 50*0.5 + 30*0.5 = 40
        assert!((result[4] - 40.0).abs() < 1e-10);
    }

    #[test]
    fn test_compute_rma() {
        let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let result = compute_rma(&data, 3);
        assert_eq!(result.len(), 5);
        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        // Seed = SMA(10,20,30) = 20.0
        assert!((result[2] - 20.0).abs() < 1e-10);
        // RMA = (20*2 + 40)/3 = 80/3 = 26.667
        assert!((result[3] - 80.0 / 3.0).abs() < 1e-10);
        // RMA = (26.667*2 + 50)/3 = 103.333/3 = 34.444
        let expected = (80.0 / 3.0 * 2.0 + 50.0) / 3.0;
        assert!((result[4] - expected).abs() < 1e-10);
    }

    #[test]
    fn test_sma_insufficient_data() {
        let data = vec![1.0, 2.0];
        let result = compute_sma(&data, 5);
        assert!(result.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn test_sma_nan_helper() {
        let data = vec![
            f64::NAN,
            f64::NAN,
            10.0,
            20.0,
            30.0,
            40.0,
        ];
        let result = sma_nan(&data, 3);
        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert!(result[2].is_nan());
        assert!(result[3].is_nan());
        // First valid: indices 2,3,4 -> 20.0
        assert!((result[4] - 20.0).abs() < 1e-10);
        assert!((result[5] - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_ema_of_nan_series() {
        let data = vec![
            f64::NAN,
            f64::NAN,
            10.0,
            20.0,
            30.0,
            40.0,
        ];
        let result = ema_of_nan_series(&data, 3);
        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert!(result[2].is_nan());
        assert!(result[3].is_nan());
        // Seed at index 4: SMA(10,20,30)=20
        assert!((result[4] - 20.0).abs() < 1e-10);
        // EMA: 40 * 0.5 + 20 * 0.5 = 30
        assert!((result[5] - 30.0).abs() < 1e-10);
    }
}
