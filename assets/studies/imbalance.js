/**
 * Imbalance
 *
 * Identifies diagonal imbalances between adjacent price levels.
 * Compares buy volume at price N+1 vs sell volume at price N
 * (and vice versa). Marks levels where the ratio exceeds threshold.
 *
 * Output: background horizontal levels
 */
indicator("Imbalance", {
    overlay: true, placement: "background", category: "orderflow",
});

const threshold = input.float("Threshold", 3.0, {
    min: 1.0, max: 10.0, step: 0.5,
});
const buyColor = input.color("Buy Color", "#4CAF50");
const sellColor = input.color("Sell Color", "#F44336");

const step = tickSize;

if (step > 0 && barCount > 0) {
    const profileBuy = {};
    const profileSell = {};

    for (let i = 0; i < barCount; i++) {
        const lo = low[i];
        const hi = high[i];
        if (hi < lo) continue;

        const numLevels = math.floor((hi - lo) / step) + 1;
        if (numLevels <= 0) continue;

        const buyPerLevel = buyVolume[i] / numLevels;
        const sellPerLevel = sellVolume[i] / numLevels;

        for (let p = lo; p <= hi; p += step) {
            const key = math.round(p / step) * step;
            if (!profileBuy[key]) profileBuy[key] = 0;
            if (!profileSell[key]) profileSell[key] = 0;
            profileBuy[key] += buyPerLevel;
            profileSell[key] += sellPerLevel;
        }
    }

    const prices = Object.keys(profileBuy)
        .map(Number)
        .sort((a, b) => a - b);

    for (let i = 0; i < prices.length - 1; i++) {
        const priceLow = prices[i];
        const priceHigh = prices[i + 1];
        const sellQty = profileSell[priceLow] || 0;
        const diagBuyQty = profileBuy[priceHigh] || 0;

        if (sellQty <= 0 || diagBuyQty <= 0) continue;

        if (diagBuyQty >= sellQty && diagBuyQty / sellQty >= threshold) {
            hline(priceHigh, "", { color: buyColor, style: "solid" });
        }
        if (sellQty >= diagBuyQty && sellQty / diagBuyQty >= threshold) {
            hline(priceLow, "", { color: sellColor, style: "solid" });
        }
    }
}

export {};
