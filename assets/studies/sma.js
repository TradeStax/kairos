/**
 * Simple Moving Average (SMA)
 *
 * Arithmetic mean of the last N data points. Smooths price data
 * to identify trend direction. SMA(i) = sum(source[i-N+1..i]) / N.
 *
 * Output: overlay line
 */
indicator("Simple Moving Average", { overlay: true, category: "trend" });

const period = input.int("Period", 20, { min: 2, max: 500 });
const source = input.source("Source", close);
const color = input.color("Color", "#2196F3");
const width = input.float("Width", 1.5, { min: 0.5, max: 5.0, step: 0.5 });
const style = input.lineStyle("Style", "solid");

const smaValues = ta.sma(source, period);

plot(smaValues, "SMA", { color, lineWidth: width, style });

export {};
