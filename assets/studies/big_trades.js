/**
 * Big Trades
 *
 * Aggregates consecutive same-side fills within a time window into
 * blocks. Renders blocks exceeding a volume threshold as sized
 * markers at the VWAP price.
 *
 * Output: overlay markers
 */
indicator("Big Trades", { overlay: true, category: "orderflow" });

const minContracts = input.int("Min Contracts", 50, { min: 1, max: 10000 });
const aggregationWindowMs = input.int("Aggregation Window (ms)", 150, {
    min: 10, max: 5000,
});
const buyColor = input.color("Buy Color", "#00CC6688");
const sellColor = input.color("Sell Color", "#FF333388");
const showLabels = input.bool("Show Labels", true);
const bubbleScale = input.float("Bubble Scale", 1.0, {
    min: 0.5, max: 3.0, step: 0.1,
});

setMarkerRenderConfig({
    shape: "circle",
    hollow: false,
    stdDev: 2.0,
    minSize: 8.0,
    maxSize: 36.0,
    minOpacity: 0.10,
    maxOpacity: 0.60,
    showText: true,
    textSize: 10.0,
    textColor: "#E0E0E0E6",
});

if (trades && trades.length > 0) {
    const blocks = [];
    let currentBlock = null;

    for (let i = 0; i < trades.length; i++) {
        const trade = trades[i];
        const qty = trade.quantity;
        if (qty <= 0) continue;

        if (currentBlock !== null
            && currentBlock.isBuy === trade.isBuy
            && (trade.time - currentBlock.lastTime) <= aggregationWindowMs) {
            currentBlock.vwapNumerator += trade.price * qty;
            currentBlock.totalQty += qty;
            currentBlock.lastTime = trade.time;
        } else {
            if (currentBlock !== null) blocks.push(currentBlock);
            currentBlock = {
                isBuy: trade.isBuy,
                vwapNumerator: trade.price * qty,
                totalQty: qty,
                firstTime: trade.time,
                lastTime: trade.time,
            };
        }
    }
    if (currentBlock !== null) blocks.push(currentBlock);

    for (let i = 0; i < blocks.length; i++) {
        const block = blocks[i];
        if (block.totalQty < minContracts) continue;

        const vwapPrice = block.vwapNumerator / block.totalQty;
        const midTime = (block.firstTime + block.lastTime) / 2;
        const color = block.isBuy ? buyColor : sellColor;

        let label = undefined;
        if (showLabels) {
            label = block.totalQty >= 1000
                ? (block.totalQty / 1000).toFixed(1) + "K"
                : String(math.floor(block.totalQty));
        }

        marker(midTime, vwapPrice, {
            size: math.sqrt(block.totalQty) * bubbleScale,
            color,
            label,
            isBuy: block.isBuy,
        });
    }
}

export {};
