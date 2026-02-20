indicator("Cumulative Volume Delta", { overlay: false, category: "volume" });

const color = input.color("color", "#2196F3", { label: "Color" });
const width = input.float("width", 1.5, { min: 0.5, max: 5.0, step: 0.5, label: "Width" });
const resetPeriod = input.choice("reset_period", "None", { options: ["None", "Daily", "Weekly"], label: "Reset Period" });

const result = ta.cvd({ reset: resetPeriod });

plot(result, "CVD", { color, lineWidth: width });

export {};
