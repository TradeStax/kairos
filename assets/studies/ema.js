/**
 * Exponential Moving Average (EMA)
 *
 * Weighted moving average giving more weight to recent prices.
 * Multiplier = 2/(N+1). EMA(i) = source(i)*mult + EMA(i-1)*(1-mult).
 * Seeded with SMA for the first N bars.
 *
 * Output: overlay line
 */
indicator("Exponential Moving Average", { overlay: true, category: "trend" });

const period = input.int("Period", 9, { min: 2, max: 500 });
const source = input.source("Source", close);
const color = input.color("Color", "#FF9800");
const width = input.float("Width", 1.5, { min: 0.5, max: 5.0, step: 0.5 });
const style = input.lineStyle("Style", "solid");

const emaValues = ta.ema(source, period);

plot(emaValues, "EMA", { color, lineWidth: width, style });

export {};
