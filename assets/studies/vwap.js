/**
 * Volume Weighted Average Price (VWAP)
 *
 * Cumulative (typical_price * volume) / cumulative(volume), where
 * tp = (H + L + C) / 3. Optional standard deviation bands.
 *
 * Output: overlay line(s) with optional fill
 */
indicator("VWAP", { overlay: true, category: "trend" });

const color = input.color("Color", "#00BCD4");
const width = input.float("Width", 1.5, { min: 0.5, max: 5.0, step: 0.5 });
const showBands = input.bool("Show Bands", false);
const bandMultiplier = input.float("Band Multiplier", 1.0, {
    min: 0.5, max: 3.0, step: 0.5,
});

if (showBands) {
    const result = ta.vwapBands(high, low, close, volume, bandMultiplier);
    plot(result.vwap, "VWAP", { color, lineWidth: width });
    const p2 = plot(result.upper, "Upper", {
        color, lineWidth: width * 0.7, style: "dashed",
    });
    const p3 = plot(result.lower, "Lower", {
        color, lineWidth: width * 0.7, style: "dashed",
    });
    fill(p2, p3, { color, opacity: 0.05 });
} else {
    const vwapValues = ta.vwap(high, low, close, volume);
    plot(vwapValues, "VWAP", { color, lineWidth: width });
}

export {};
