indicator("Volume", { overlay: false, category: "volume" });

const upColor = input.color("up_color", "#4CAF50", { label: "Up Color" });
const downColor = input.color("down_color", "#F44336", { label: "Down Color" });
const opacity = input.float("opacity", 0.8, { min: 0, max: 1, step: 0.05, label: "Opacity" });

const colors = new Array(barCount);
for (let i = 0; i < barCount; i++) {
    colors[i] = close[i] >= open[i] ? upColor : downColor;
}

plotBar(volume, "Volume", { color: colors, opacity });

export {};
