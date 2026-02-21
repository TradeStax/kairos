/**
 * Stochastic Oscillator
 *
 * Momentum oscillator comparing closing price to the high-low range.
 * Raw %K = 100 * (C - LL) / (HH - LL). Slow %K = SMA(raw, smooth).
 * %D = SMA(%K, dPeriod). Ranges 0-100.
 *
 * Output: panel lines with overbought/oversold levels
 */
indicator("Stochastic", { overlay: false, category: "momentum" });

const kPeriod = input.int("%K Period", 14, { min: 5, max: 50 });
const dPeriod = input.int("%D Period", 3, { min: 1, max: 20 });
const smooth = input.int("Smooth", 3, { min: 1, max: 10 });
const overbought = input.float("Overbought", 80, {
    min: 50, max: 100, step: 5,
});
const oversold = input.float("Oversold", 20, {
    min: 0, max: 50, step: 5,
});
const kColor = input.color("%K Color", "#2196F3");
const dColor = input.color("%D Color", "#F44336");

const result = ta.stoch(high, low, close, kPeriod, dPeriod, smooth);

plot(result.k, "%K", { color: kColor, lineWidth: 1.5 });
plot(result.d, "%D", { color: dColor, lineWidth: 1.5, style: "dashed" });
hline(overbought, "Overbought", { color: "#787B86", style: "dashed" });
hline(oversold, "Oversold", { color: "#787B86", style: "dashed" });

export {};
