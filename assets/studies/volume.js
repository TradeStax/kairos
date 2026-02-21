/**
 * Volume
 *
 * Total volume per candle displayed as colored bars. Green for
 * up-closes (close >= open), red for down-closes.
 *
 * Output: panel bars
 */
indicator("Volume", { overlay: false, category: "volume" });

const upColor = input.color("Up Color", "#4CAF50");
const downColor = input.color("Down Color", "#F44336");

const colors = new Array(barCount);
for (let i = 0; i < barCount; i++) {
    colors[i] = close[i] >= open[i] ? upColor : downColor;
}

plotBarColored(volume, colors, "Volume");

export {};
