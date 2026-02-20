indicator("Value Area", { overlay: true, category: "orderflow", placement: "background" });

const percentage = input.float("percentage", 0.7, { min: 0.5, max: 0.95, step: 0.05, label: "Percentage" });
const vahColor = input.color("vah_color", "#4CAF50", { label: "VAH Color" });
const valColor = input.color("val_color", "#F44336", { label: "VAL Color" });
const fillOpacity = input.float("fill_opacity", 0.1, { min: 0, max: 0.5, step: 0.05, label: "Fill Opacity" });

// Build volume profile from all candle data
const profileMap = {};
const step = tickSize;

if (step > 0 && barCount > 0) {
    for (let i = 0; i < barCount; i++) {
        const lo = low[i];
        const hi = high[i];
        const vol = volume[i];

        if (hi < lo) continue;

        const numLevels = math.floor((hi - lo) / step) + 1;
        if (numLevels <= 0) continue;

        const volPerLevel = vol / numLevels;

        for (let p = lo; p <= hi; p += step) {
            const key = math.round(p / step) * step;
            if (!profileMap[key]) {
                profileMap[key] = 0;
            }
            profileMap[key] += volPerLevel;
        }
    }

    const prices = Object.keys(profileMap).map(Number).sort((a, b) => a - b);

    if (prices.length > 0) {
        // Find POC
        let pocIdx = 0;
        let maxVol = 0;
        const volumes = [];

        for (let i = 0; i < prices.length; i++) {
            const v = profileMap[prices[i]];
            volumes.push(v);
            if (v > maxVol) {
                maxVol = v;
                pocIdx = i;
            }
        }

        // Expand from POC to capture target percentage of total volume
        let totalVolume = 0;
        for (let i = 0; i < volumes.length; i++) {
            totalVolume += volumes[i];
        }

        const target = totalVolume * percentage;
        let accumulated = volumes[pocIdx];
        let upper = pocIdx;
        let lower = pocIdx;

        while (accumulated < target && (lower > 0 || upper < prices.length - 1)) {
            const upVol = upper + 1 < prices.length ? volumes[upper + 1] : 0;
            const downVol = lower > 0 ? volumes[lower - 1] : 0;

            if (upVol >= downVol && upper + 1 < prices.length) {
                upper += 1;
                accumulated += upVol;
            } else if (lower > 0) {
                lower -= 1;
                accumulated += downVol;
            } else if (upper + 1 < prices.length) {
                upper += 1;
                accumulated += upVol;
            } else {
                break;
            }
        }

        const vahPrice = prices[upper];
        const valPrice = prices[lower];

        // Create constant line arrays spanning all bars
        const vahLine = new Array(barCount);
        const valLine = new Array(barCount);
        for (let i = 0; i < barCount; i++) {
            vahLine[i] = vahPrice;
            valLine[i] = valPrice;
        }

        const p1 = plot(vahLine, "VAH", { color: vahColor, lineWidth: 1.0, style: "dashed" });
        const p2 = plot(valLine, "VAL", { color: valColor, lineWidth: 1.0, style: "dashed" });
        fill(p1, p2, { color: vahColor, opacity: fillOpacity });
    }
}

export {};
