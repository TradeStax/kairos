indicator("Simple Moving Average", { overlay: true, category: "trend" });

const period = input.int("period", 20, { min: 2, max: 500, label: "Period" });
const source = input.source("source", "close", { label: "Source" });
const color = input.color("color", "#2196F3", { label: "Color" });
const width = input.float("width", 1.5, { min: 0.5, max: 5.0, step: 0.5, label: "Width" });
const style = input.lineStyle("style", "solid", { label: "Style" });

const smaValues = ta.sma(source, period);

plot(smaValues, "SMA", { color, lineWidth: width, style });

export {};
