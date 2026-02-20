indicator("On Balance Volume", { overlay: false, category: "volume" });

const color = input.color("color", "#FFFFFF", { label: "Color" });
const width = input.float("width", 1.5, { min: 0.5, max: 5.0, step: 0.5, label: "Width" });

const obvValues = ta.obv();

plot(obvValues, "OBV", { color, lineWidth: width });

export {};
