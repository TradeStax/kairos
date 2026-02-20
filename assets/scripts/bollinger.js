indicator("Bollinger Bands", { overlay: true, category: "volatility" });

const period = input.int("period", 20, { min: 2, max: 500, label: "Period" });
const stdDev = input.float("std_dev", 2.0, { min: 0.5, max: 5.0, step: 0.5, label: "Std Dev" });
const color = input.color("color", "#2196F3", { label: "Color" });
const fillOpacity = input.float("fill_opacity", 0.1, { min: 0, max: 1, step: 0.05, label: "Fill Opacity" });

const result = ta.bb(close, period, stdDev);

const p1 = plot(result.upper, "Upper", { color, lineWidth: 1.0 });
plot(result.middle, "BB", { color, lineWidth: 1.0 });
const p2 = plot(result.lower, "Lower", { color, lineWidth: 1.0 });
fill(p1, p2, { color, opacity: fillOpacity });

export {};
