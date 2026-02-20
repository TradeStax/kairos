indicator("Volume Delta", { overlay: false, category: "volume" });

const positiveColor = input.color("positive_color", "#4CAF50", { label: "Positive Color" });
const negativeColor = input.color("negative_color", "#F44336", { label: "Negative Color" });
const opacity = input.float("opacity", 0.8, { min: 0, max: 1, step: 0.05, label: "Opacity" });

const deltaValues = new Array(barCount);
const colors = new Array(barCount);
for (let i = 0; i < barCount; i++) {
    deltaValues[i] = buyVolume[i] - sellVolume[i];
    colors[i] = deltaValues[i] >= 0 ? positiveColor : negativeColor;
}

plotBar(deltaValues, "Delta", { color: colors, opacity });

export {};
