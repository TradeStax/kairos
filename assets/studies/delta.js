/**
 * Volume Delta
 *
 * Per-candle difference between buy and sell volume. Positive
 * values indicate net buying, negative indicate net selling.
 * delta(i) = buyVolume(i) - sellVolume(i).
 *
 * Output: panel bars
 */
indicator("Volume Delta", { overlay: false, category: "volume" });

const positiveColor = input.color("Positive Color", "#4CAF50");
const negativeColor = input.color("Negative Color", "#F44336");

const deltaValues = new Array(barCount);
const colors = new Array(barCount);
for (let i = 0; i < barCount; i++) {
    deltaValues[i] = buyVolume[i] - sellVolume[i];
    colors[i] = deltaValues[i] >= 0 ? positiveColor : negativeColor;
}

plotBarColored(deltaValues, colors, "Delta");

export {};
