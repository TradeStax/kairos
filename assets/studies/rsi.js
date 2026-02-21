/**
 * Relative Strength Index (RSI)
 *
 * Momentum oscillator measuring speed of price changes. Ranges
 * 0-100. RSI = 100 - 100/(1 + avgGain/avgLoss). Uses Wilder's
 * smoothing (RMA) for gain/loss averages.
 *
 * Output: panel line with overbought/oversold levels
 */
indicator("RSI", { overlay: false, category: "momentum" });

const period = input.int("Period", 14, { min: 2, max: 100 });
const overbought = input.float("Overbought", 70, {
    min: 50, max: 100, step: 5,
});
const oversold = input.float("Oversold", 30, {
    min: 0, max: 50, step: 5,
});
const color = input.color("Color", "#9C27B0");

const rsiValues = ta.rsi(close, period);

plot(rsiValues, "RSI", { color, lineWidth: 1.5 });
hline(overbought, "Overbought", { color: "#787B86", style: "dashed" });
hline(50, "Midline", { color: "#787B8640", style: "dotted" });
hline(oversold, "Oversold", { color: "#787B86", style: "dashed" });

export {};
