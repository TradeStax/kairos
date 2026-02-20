indicator("Big Trades", { overlay: true, category: "orderflow" });

const minContracts = input.int("min_contracts", 50, { min: 1, max: 10000, label: "Min Contracts" });
const aggregationWindowMs = input.int("aggregation_window_ms", 150, { min: 10, max: 5000, label: "Aggregation Window (ms)" });
const buyColor = input.color("buy_color", "#00CC6688", { label: "Buy Color" });
const sellColor = input.color("sell_color", "#FF333388", { label: "Sell Color" });
const showLabels = input.bool("show_labels", true, { label: "Show Labels" });
const bubbleScale = input.float("bubble_scale", 1.0, { min: 0.5, max: 3.0, step: 0.1, label: "Bubble Scale" });

// Aggregate consecutive same-side fills within the time window
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
            // Merge into current block
            currentBlock.vwapNumerator += trade.price * qty;
            currentBlock.totalQty += qty;
            currentBlock.lastTime = trade.time;
            currentBlock.fillCount += 1;
        } else {
            // Flush current block and start new one
            if (currentBlock !== null) {
                blocks.push(currentBlock);
            }
            currentBlock = {
                isBuy: trade.isBuy,
                vwapNumerator: trade.price * qty,
                totalQty: qty,
                firstTime: trade.time,
                lastTime: trade.time,
                fillCount: 1
            };
        }
    }

    // Flush final block
    if (currentBlock !== null) {
        blocks.push(currentBlock);
    }

    // Output markers for blocks meeting the threshold
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
            isBuy: block.isBuy
        });
    }
}

export {};
