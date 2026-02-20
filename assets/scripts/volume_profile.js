indicator("Volume Profile", { overlay: false, category: "orderflow", placement: "background" });

const widthPct = input.float("width_pct", 0.3, { min: 0.05, max: 0.5, step: 0.05, label: "Width %" });
const pocColor = input.color("poc_color", "#FFD700", { label: "POC Color" });
const valColor = input.color("val_color", "#4D99FF80", { label: "Value Area Color" });
const varColor = input.color("var_color", "#8080804D", { label: "Volume Color" });

// Build volume profile from candle data
const profileMap = {};

for (let i = 0; i < barCount; i++) {
    const lo = low[i];
    const hi = high[i];
    const vol = volume[i];
    const step = tickSize;

    if (step <= 0 || hi < lo) continue;

    const numLevels = math.floor((hi - lo) / step) + 1;
    if (numLevels <= 0) continue;

    const buyPerLevel = buyVolume[i] / numLevels;
    const sellPerLevel = sellVolume[i] / numLevels;

    for (let p = lo; p <= hi; p += step) {
        const key = math.round(p / step) * step;
        if (!profileMap[key]) {
            profileMap[key] = { buy: 0, sell: 0 };
        }
        profileMap[key].buy += buyPerLevel;
        profileMap[key].sell += sellPerLevel;
    }
}

// Find POC (max volume level)
let pocPrice = 0;
let maxVol = 0;
const prices = Object.keys(profileMap).map(Number).sort((a, b) => a - b);

for (let i = 0; i < prices.length; i++) {
    const p = prices[i];
    const total = profileMap[p].buy + profileMap[p].sell;
    if (total > maxVol) {
        maxVol = total;
        pocPrice = p;
    }
}

// Calculate value area (70% of total volume centered on POC)
if (prices.length > 0 && maxVol > 0) {
    let totalVolume = 0;
    const volumes = [];
    for (let i = 0; i < prices.length; i++) {
        const v = profileMap[prices[i]].buy + profileMap[prices[i]].sell;
        volumes.push(v);
        totalVolume += v;
    }

    const pocIdx = prices.indexOf(pocPrice);
    const target = totalVolume * 0.7;
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

    hline(pocPrice, "POC", { color: pocColor, style: "solid" });
    hline(prices[upper], "VAH", { color: valColor, style: "dashed" });
    hline(prices[lower], "VAL", { color: valColor, style: "dashed" });
}

export {};
