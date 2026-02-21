/**
 * Value Area
 *
 * Computes the price range containing a given percentage of total
 * volume (default 70%). Draws VAH and VAL lines with fill.
 * Uses ta.valueArea() for Rust-optimized computation.
 *
 * Output: background band (VAH/VAL lines with fill)
 */
indicator("Value Area", {
    overlay: true, placement: "background", category: "orderflow",
});

const percentage = input.float("Percentage", 0.7, {
    min: 0.5, max: 0.95, step: 0.05,
});
const vahColor = input.color("VAH Color", "#4CAF50");
const valColor = input.color("VAL Color", "#F44336");
const fillOpacity = input.float("Fill Opacity", 0.1, {
    min: 0, max: 0.5, step: 0.05,
});

const va = ta.valueArea(high, low, volume, tickSize, percentage);

if (va && barCount > 0) {
    const vahLine = new Array(barCount);
    const valLine = new Array(barCount);
    for (let i = 0; i < barCount; i++) {
        vahLine[i] = va.vah;
        valLine[i] = va.val;
    }
    const p1 = plot(vahLine, "VAH", {
        color: vahColor, lineWidth: 1.0, style: "dashed",
    });
    const p2 = plot(valLine, "VAL", {
        color: valColor, lineWidth: 1.0, style: "dashed",
    });
    fill(p1, p2, { color: vahColor, opacity: fillOpacity });
}

export {};
