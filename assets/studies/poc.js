/**
 * Point of Control (POC)
 *
 * Rolling POC line: the price level with highest volume over a
 * lookback window. Uses ta.rollingPoc() for efficient computation.
 *
 * Output: overlay line
 */
indicator("Point of Control", { overlay: true, category: "orderflow" });

const lookback = input.int("Lookback", 20, { min: 1, max: 500 });
const color = input.color("Color", "#FFD700");
const width = input.float("Width", 1.5, { min: 0.5, max: 5.0, step: 0.5 });

const pocValues = ta.rollingPoc(high, low, volume, tickSize, lookback);

plot(pocValues, "POC", { color, lineWidth: width });

export {};
