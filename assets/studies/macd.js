/**
 * Moving Average Convergence Divergence (MACD)
 *
 * Trend-following momentum indicator. MACD = EMA(fast) - EMA(slow).
 * Signal = EMA(MACD, signal_period). Histogram = MACD - Signal.
 *
 * Output: panel lines + histogram
 */
indicator("MACD", { overlay: false, category: "momentum" });

const fastPeriod = input.int("Fast Period", 12, { min: 2, max: 100 });
const slowPeriod = input.int("Slow Period", 26, { min: 2, max: 200 });
const signalPeriod = input.int("Signal Period", 9, { min: 2, max: 100 });
const macdColor = input.color("MACD Color", "#2196F3");
const signalColor = input.color("Signal Color", "#FF9800");
const histPosColor = input.color("Histogram +", "#4CAF50");
const histNegColor = input.color("Histogram -", "#F44336");

const result = ta.macd(close, fastPeriod, slowPeriod, signalPeriod);

plot(result.macd, "MACD", { color: macdColor, lineWidth: 1.5 });
plot(result.signal, "Signal", { color: signalColor, lineWidth: 1.5 });

const histColors = new Array(barCount);
for (let i = 0; i < barCount; i++) {
    histColors[i] = result.histogram[i] >= 0 ? histPosColor : histNegColor;
}
plotHistogramColored(result.histogram, histColors, "Histogram");

export {};
