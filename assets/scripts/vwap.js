indicator("VWAP", { overlay: true, category: "trend" });

const color = input.color("color", "#00BCD4", { label: "Color" });
const width = input.float("width", 1.5, { min: 0.5, max: 5.0, step: 0.5, label: "Width" });
const showBands = input.bool("show_bands", false, { label: "Show Bands" });
const bandMultiplier = input.float("band_multiplier", 1.0, { min: 1.0, max: 3.0, step: 0.5, label: "Band Multiplier" });

const result = ta.vwap({ bands: showBands, multiplier: bandMultiplier });

const p1 = plot(result.vwap, "VWAP", { color, lineWidth: width });

if (showBands) {
    const p2 = plot(result.upper, "VWAP Upper", { color, lineWidth: width * 0.7, style: "dashed" });
    const p3 = plot(result.lower, "VWAP Lower", { color, lineWidth: width * 0.7, style: "dashed" });
    fill(p2, p3, { color, opacity: 0.05 });
}

export {};
