/**
 * Bollinger Bands
 *
 * SMA-based envelope with standard deviation bands.
 * Middle = SMA(close, period). Upper/Lower = Middle +/- mult * stdev.
 * Measures volatility and identifies overbought/oversold conditions.
 *
 * Output: overlay band (upper + middle + lower with fill)
 */
indicator("Bollinger Bands", { overlay: true, category: "volatility" });

const period = input.int("Period", 20, { min: 2, max: 500 });
const stdDev = input.float("Std Dev", 2.0, {
    min: 0.5, max: 5.0, step: 0.5,
});
const color = input.color("Color", "#2196F3");
const fillOpacity = input.float("Fill Opacity", 0.1, {
    min: 0, max: 1, step: 0.05,
});

const result = ta.bb(close, period, stdDev);

const p1 = plot(result.upper, "Upper", { color, lineWidth: 1.0 });
plot(result.middle, "BB", { color, lineWidth: 1.0 });
const p2 = plot(result.lower, "Lower", { color, lineWidth: 1.0 });
fill(p1, p2, { color, opacity: fillOpacity });

export {};
