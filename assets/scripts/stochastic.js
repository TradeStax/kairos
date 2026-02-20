indicator("Stochastic", { overlay: false, category: "momentum" });

const kPeriod = input.int("k_period", 14, { min: 5, max: 50, label: "%K Period" });
const dPeriod = input.int("d_period", 3, { min: 1, max: 20, label: "%D Period" });
const smooth = input.int("smooth", 3, { min: 1, max: 10, label: "Smooth" });
const overbought = input.float("overbought", 80, { min: 50, max: 100, step: 5, label: "Overbought" });
const oversold = input.float("oversold", 20, { min: 0, max: 50, step: 5, label: "Oversold" });
const kColor = input.color("k_color", "#2196F3", { label: "%K Color" });
const dColor = input.color("d_color", "#F44336", { label: "%D Color" });

const result = ta.stoch(high, low, close, kPeriod, dPeriod, smooth);

plot(result.k, "%K", { color: kColor, lineWidth: 1.5 });
plot(result.d, "%D", { color: dColor, lineWidth: 1.5, style: "dashed" });
hline(overbought, "Overbought", { color: "#787B86", style: "dashed" });
hline(oversold, "Oversold", { color: "#787B86", style: "dashed" });

export {};
