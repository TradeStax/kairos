indicator("Exponential Moving Average", { overlay: true, category: "trend" });

const period = input.int("period", 9, { min: 2, max: 500, label: "Period" });
const source = input.source("source", "close", { label: "Source" });
const color = input.color("color", "#FF9800", { label: "Color" });
const width = input.float("width", 1.5, { min: 0.5, max: 5.0, step: 0.5, label: "Width" });
const style = input.lineStyle("style", "solid", { label: "Style" });

const emaValues = ta.ema(source, period);

plot(emaValues, "EMA", { color, lineWidth: width, style });

export {};
