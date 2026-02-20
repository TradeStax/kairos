indicator("Imbalance", { overlay: true, category: "orderflow", placement: "background" });

const threshold = input.float("threshold", 3.0, { min: 1.0, max: 10.0, step: 0.5, label: "Threshold" });
const buyColor = input.color("buy_color", "#4CAF50", { label: "Buy Color" });
const sellColor = input.color("sell_color", "#F44336", { label: "Sell Color" });

// Build a buy/sell volume profile from candle data
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

    // Check adjacent levels for diagonal imbalances
    const prices = Object.keys(profileBuy).map(Number).sort((a, b) => a - b);

    for (let i = 0; i < prices.length - 1; i++) {
        const priceLow = prices[i];
        const priceHigh = prices[i + 1];

        const sellQty = profileSell[priceLow] || 0;
        const diagBuyQty = profileBuy[priceHigh] || 0;

        // Skip zero volumes
        if (sellQty <= 0 || diagBuyQty <= 0) continue;

        // Check for buy imbalance (diagonal buy >> sell)
        if (diagBuyQty >= sellQty && sellQty > 0) {
            const ratio = diagBuyQty / sellQty;
            if (ratio >= threshold) {
                hline(priceHigh, "", { color: buyColor, style: "solid" });
            }
        }

        // Check for sell imbalance (sell >> diagonal buy)
        if (sellQty >= diagBuyQty && diagBuyQty > 0) {
            const ratio = sellQty / diagBuyQty;
            if (ratio >= threshold) {
                hline(priceLow, "", { color: sellColor, style: "solid" });
            }
        }
    }
}

export {};
