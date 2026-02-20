indicator("Point of Control", { overlay: true, category: "orderflow" });

const lookback = input.int("lookback", 20, { min: 1, max: 500, label: "Lookback" });
const color = input.color("color", "#FFD700", { label: "Color" });
const width = input.float("width", 1.5, { min: 0.5, max: 5.0, step: 0.5, label: "Width" });

// Compute rolling POC: for each bar, build a volume profile over the lookback
// window and find the price with the highest volume
const pocValues = new Array(barCount);

for (let i = 0; i < barCount; i++) {
    if (i < lookback - 1) {
        pocValues[i] = NaN;
        continue;
    }

    const start = i - lookback + 1;
    const profileMap = {};
    const step = tickSize;

    if (step <= 0) {
        pocValues[i] = NaN;
        continue;
    }

    for (let j = start; j <= i; j++) {
        const lo = low[j];
        const hi = high[j];
        const vol = volume[j];

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

    // Find the price level with highest volume
    let maxVol = 0;
    let maxPrice = close[i];
    const prices = Object.keys(profileMap);

    for (let k = 0; k < prices.length; k++) {
        const p = Number(prices[k]);
        if (profileMap[p] > maxVol) {
            maxVol = profileMap[p];
            maxPrice = p;
        }
    }

    pocValues[i] = maxPrice;
}

plot(pocValues, "POC", { color, lineWidth: width });

export {};
