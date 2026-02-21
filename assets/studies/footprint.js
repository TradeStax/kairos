/**
 * Footprint
 *
 * Per-candle trade volume at each price level. Replaces standard
 * candle rendering with detailed buy/sell volume bars. Supports
 * multiple display modes: Profile, Box; data types: Volume,
 * Bid/Ask Split, Delta, Delta+Volume.
 *
 * Output: candle replace (footprint)
 */
indicator("Footprint", {
    placement: "candle_replace", category: "orderflow",
});

const mode = input.choice("Mode", "Profile", {
    options: ["Profile", "Box"],
});
const dataType = input.choice("Data Type", "Volume", {
    options: ["Volume", "BidAskSplit", "Delta", "DeltaAndVolume"],
});
const scaling = input.choice("Scaling", "Sqrt", {
    options: ["Linear", "Sqrt", "Log", "VisibleRange", "Datapoint"],
});
const candlePosition = input.choice("Candle Position", "Left", {
    options: ["None", "Left", "Center", "Right"],
});

setCandleRenderConfig({
    defaultCellWidth: 80,
    maxCellWidth: 500,
    minCellWidth: 10,
    cellHeightRatio: 4,
    initialCandleWindow: 12,
    autoscaleXCells: 1.0,
});

const fp = ta.buildFootprint(
    { time, open, high, low, close },
    trades,
    tickSize,
);

plotFootprint(fp, { mode, dataType, scaling, candlePosition });

export {};
