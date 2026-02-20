indicator("RSI", { overlay: false, category: "momentum" });

const period = input.int("period", 14, { min: 2, max: 100, label: "Period" });
const overbought = input.float("overbought", 70, { min: 50, max: 100, step: 5, label: "Overbought" });
const oversold = input.float("oversold", 30, { min: 0, max: 50, step: 5, label: "Oversold" });
const color = input.color("color", "#9C27B0", { label: "Color" });

const rsiValues = ta.rsi(close, period);

plot(rsiValues, "RSI", { color, lineWidth: 1.5 });
hline(overbought, "Overbought", { color: "#787B86", style: "dashed" });
hline(50, "Midline", { color: "#787B8640", style: "dotted" });
hline(oversold, "Oversold", { color: "#787B86", style: "dashed" });

export {};
