indicator("ATR", { overlay: false, category: "volatility" });

const period = input.int("period", 14, { min: 1, max: 100, label: "Period" });
const color = input.color("color", "#FF9800", { label: "Color" });
const width = input.float("width", 1.5, { min: 0.5, max: 5.0, step: 0.5, label: "Width" });

const atrValues = ta.atr(period);

plot(atrValues, "ATR", { color, lineWidth: width });

export {};
