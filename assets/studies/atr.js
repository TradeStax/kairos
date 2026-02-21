/**
 * Average True Range (ATR)
 *
 * Measures volatility using true range (max of H-L, |H-Pc|, |L-Pc|)
 * smoothed with Wilder's RMA. Higher values indicate higher volatility.
 *
 * Output: panel line
 */
indicator("ATR", { overlay: false, category: "volatility" });

const period = input.int("Period", 14, { min: 1, max: 100 });
const color = input.color("Color", "#FF9800");
const width = input.float("Width", 1.5, { min: 0.5, max: 5.0, step: 0.5 });

const atrValues = ta.atr(high, low, close, period);

plot(atrValues, "ATR", { color, lineWidth: width });

export {};
