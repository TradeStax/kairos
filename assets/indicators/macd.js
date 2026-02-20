indicator("MACD", { overlay: false, category: "momentum" });

const fastPeriod = input.int("fast_period", 12, { min: 2, max: 100, label: "Fast Period" });
const slowPeriod = input.int("slow_period", 26, { min: 2, max: 200, label: "Slow Period" });
const signalPeriod = input.int("signal_period", 9, { min: 2, max: 100, label: "Signal Period" });
const macdColor = input.color("macd_color", "#2196F3", { label: "MACD Color" });
const signalColor = input.color("signal_color", "#FF9800", { label: "Signal Color" });
const histPosColor = input.color("hist_positive_color", "#4CAF50", { label: "Histogram +" });
const histNegColor = input.color("hist_negative_color", "#F44336", { label: "Histogram -" });

const result = ta.macd(close, fastPeriod, slowPeriod, signalPeriod);

plot(result.macd, "MACD", { color: macdColor, lineWidth: 1.5 });
plot(result.signal, "Signal", { color: signalColor, lineWidth: 1.5 });

const histColors = new Array(barCount);
for (let i = 0; i < barCount; i++) {
    histColors[i] = result.histogram[i] >= 0 ? histPosColor : histNegColor;
}
plotHistogram(result.histogram, "Histogram", { color: histColors });

export {};
