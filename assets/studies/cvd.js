/**
 * Cumulative Volume Delta (CVD)
 *
 * Running sum of (buy_volume - sell_volume). Tracks net buying/selling
 * pressure. Optional daily or weekly reset for session-based analysis.
 *
 * Output: panel line
 */
indicator("Cumulative Volume Delta", { overlay: false, category: "volume" });

const color = input.color("Color", "#2196F3");
const width = input.float("Width", 1.5, { min: 0.5, max: 5.0, step: 0.5 });
const resetPeriod = input.choice("Reset Period", "None", {
    options: ["None", "Daily", "Weekly"],
});

let result;
if (resetPeriod === "None") {
    result = ta.cvd(buyVolume, sellVolume);
} else {
    result = ta.cvdReset(buyVolume, sellVolume, time, resetPeriod);
}

plot(result, "CVD", { color, lineWidth: width });

export {};
