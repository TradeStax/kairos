//! Built-in variables injected into the JS context.
//!
//! Provides: open, high, low, close, volume, buyVolume, sellVolume arrays,
//! time, barIndex, barCount, tickSize, delta, trades array, bar object.

use crate::error::ScriptError;
use data::{Candle, Price, Trade};
use rquickjs::Ctx;

/// Inject all candle data as JavaScript global arrays for full-array execution.
///
/// After this call, the JS context has:
/// - `open`, `high`, `low`, `close`: f64 arrays of prices
/// - `volume`, `buyVolume`, `sellVolume`: f64 arrays
/// - `time`: u64 array of timestamps (ms)
/// - `delta`: f64 array of (buyVolume - sellVolume)
/// - `barCount`: integer
/// - `tickSize`: f64
/// - `hl2`, `hlc3`, `ohlc4`: derived price series
pub fn inject_candle_globals(
    ctx: &Ctx<'_>,
    candles: &[Candle],
    tick_size: Price,
) -> Result<(), ScriptError> {
    let len = candles.len();
    let globals = ctx.globals();

    let mut open_arr = Vec::with_capacity(len);
    let mut high_arr = Vec::with_capacity(len);
    let mut low_arr = Vec::with_capacity(len);
    let mut close_arr = Vec::with_capacity(len);
    let mut volume_arr = Vec::with_capacity(len);
    let mut buy_vol_arr = Vec::with_capacity(len);
    let mut sell_vol_arr = Vec::with_capacity(len);
    let mut time_arr = Vec::with_capacity(len);
    let mut delta_arr = Vec::with_capacity(len);
    let mut hl2_arr = Vec::with_capacity(len);
    let mut hlc3_arr = Vec::with_capacity(len);
    let mut ohlc4_arr = Vec::with_capacity(len);

    for candle in candles {
        let o = candle.open.to_f64();
        let h = candle.high.to_f64();
        let l = candle.low.to_f64();
        let c = candle.close.to_f64();
        let bv = candle.buy_volume.0;
        let sv = candle.sell_volume.0;

        open_arr.push(o);
        high_arr.push(h);
        low_arr.push(l);
        close_arr.push(c);
        volume_arr.push(bv + sv);
        buy_vol_arr.push(bv);
        sell_vol_arr.push(sv);
        time_arr.push(candle.time.0 as f64);
        delta_arr.push(bv - sv);
        hl2_arr.push((h + l) / 2.0);
        hlc3_arr.push((h + l + c) / 3.0);
        ohlc4_arr.push((o + h + l + c) / 4.0);
    }

    globals.set("open", open_arr)?;
    globals.set("high", high_arr)?;
    globals.set("low", low_arr)?;
    globals.set("close", close_arr)?;
    globals.set("volume", volume_arr)?;
    globals.set("buyVolume", buy_vol_arr)?;
    globals.set("sellVolume", sell_vol_arr)?;
    globals.set("time", time_arr)?;
    globals.set("delta", delta_arr)?;
    globals.set("hl2", hl2_arr)?;
    globals.set("hlc3", hlc3_arr)?;
    globals.set("ohlc4", ohlc4_arr)?;
    globals.set("barCount", len)?;
    globals.set("tickSize", tick_size.to_f64())?;

    Ok(())
}

/// Inject raw trade data as a JS global array.
///
/// After this call, the JS context has:
/// - `trades`: array of objects with { time, price, quantity, isBuy }
pub fn inject_trades_global(
    ctx: &Ctx<'_>,
    trades: &[Trade],
) -> Result<(), ScriptError> {
    let globals = ctx.globals();

    // Build trades as an array of plain objects
    let trades_array = rquickjs::Array::new(ctx.clone())?;
    for (i, trade) in trades.iter().enumerate() {
        let obj = rquickjs::Object::new(ctx.clone())?;
        obj.set("time", trade.time.0 as f64)?;
        obj.set("price", trade.price.to_f64())?;
        obj.set("quantity", trade.quantity.0)?;
        obj.set("isBuy", trade.side.is_buy())?;
        trades_array.set(i, obj)?;
    }
    globals.set("trades", trades_array)?;

    Ok(())
}

/// Install stub globals for the declaration pass (no candle data).
///
/// Sets all series to empty arrays and scalar globals to defaults.
pub fn install_stub_globals(ctx: &Ctx<'_>) -> Result<(), ScriptError> {
    let globals = ctx.globals();
    let empty: Vec<f64> = vec![];
    globals.set("open", empty.clone())?;
    globals.set("high", empty.clone())?;
    globals.set("low", empty.clone())?;
    globals.set("close", empty.clone())?;
    globals.set("volume", empty.clone())?;
    globals.set("buyVolume", empty.clone())?;
    globals.set("sellVolume", empty.clone())?;
    globals.set("time", empty.clone())?;
    globals.set("delta", empty.clone())?;
    globals.set("hl2", empty.clone())?;
    globals.set("hlc3", empty.clone())?;
    globals.set("ohlc4", empty)?;
    globals.set("barCount", 0i32)?;
    globals.set("tickSize", 0.01f64)?;

    let trades_array = rquickjs::Array::new(ctx.clone())?;
    globals.set("trades", trades_array)?;

    Ok(())
}
