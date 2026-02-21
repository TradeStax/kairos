/**
 * On Balance Volume (OBV)
 *
 * Cumulative volume indicator that adds volume on up-closes and
 * subtracts on down-closes. Confirms trend strength via volume flow.
 *
 * Output: panel line
 */
indicator("On Balance Volume", { overlay: false, category: "volume" });

const color = input.color("Color", "#FFFFFF");
const width = input.float("Width", 1.5, { min: 0.5, max: 5.0, step: 0.5 });

const obvValues = ta.obv(close, volume);

plot(obvValues, "OBV", { color, lineWidth: width });

export {};
