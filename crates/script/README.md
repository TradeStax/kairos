# Kairos Script — JavaScript Indicators

The `kairos-script` crate runs custom indicator scripts written in JavaScript (QuickJS). Scripts are discovered from:

- **Bundled**: `assets/scripts/` (relative to the executable or current directory)
- **User**: `$KAIROS_DATA_PATH/scripts` or platform data dir `kairos/scripts` (e.g. `~/.local/share/kairos/scripts` on Linux)

## Script structure

1. **Declaration** — Call `indicator(name, options)` once at top level.
2. **Inputs** — Use `input.*()` to define parameters (shown in the indicator settings UI).
3. **Logic** — Use `ta.*` helpers and your own logic to compute series.
4. **Output** — Call `plot()`, `plotBar()`, `plotHistogram()`, `plotShape()`, `marker()`, `hline()`, `fill()`, etc.

### Example (SMA)

```javascript
indicator("Simple Moving Average", { overlay: true, category: "trend" });

const period = input.int("period", 20, { min: 2, max: 500, label: "Period" });
const source = input.source("source", "close", { label: "Source" });
const color = input.color("color", "#2196F3", { label: "Color" });
const width = input.float("width", 1.5, { min: 0.5, max: 5.0, step: 0.5, label: "Width" });
const style = input.lineStyle("style", "solid", { label: "Style" });

const smaValues = ta.sma(source, period);
plot(smaValues, "SMA", { color, lineWidth: width, style });

export {};
```

## API summary

### `indicator(name, options)`

- **name**: Display name in the UI.
- **options**: `{ overlay: boolean, category: string }`.
  - **overlay**: `true` = draw on price chart; `false` = separate panel.
  - **category**: One of `"trend"`, `"momentum"`, `"volume"`, `"volatility"`, `"orderflow"`.

### Inputs

- `input.int(key, default, { min, max, label })`
- `input.float(key, default, { min, max, step, label })`
- `input.source(key, default, { label })` — e.g. `"open"`, `"high"`, `"low"`, `"close"`, `"hl2"`, `"hlc3"`, `"ohlc4"`.
- `input.color(key, defaultHex, { label })`
- `input.lineStyle(key, default, { label })` — e.g. `"solid"`, `"dotted"`, `"dashed"`.
- `input.bool(key, default, { label })`
- `input.string(key, default, { label })`

### Globals (injected each bar range)

- **OHLCV**: `open`, `high`, `low`, `close`, `volume`, `buyVolume`, `sellVolume` (arrays of numbers).
- **Time**: `time` (array of timestamps in ms).
- **Derived**: `hl2`, `hlc3`, `ohlc4`, `delta` (buyVolume − sellVolume per bar).
- **Meta**: `barCount`, `tickSize`.

### Technical analysis (`ta.*`)

- `ta.sma(source, period)`
- `ta.ema(source, period)`
- Others as implemented in the runtime (see `script/src/runtime/ta.rs`).

### Plotting

- `plot(series, name, options)` — line; options: `color`, `lineWidth`, `style`.
- `plotBar(values, name, options)` — bar series.
- `plotHistogram(values, name, options)` — histogram.
- `plotShape(times, prices, options)` — shapes at (time, price).
- `marker(time, price, options)` — single marker; options: `size`, `color`, `label`, `isBuy`.
- `hline(price, name, options)` — horizontal level.
- `fill(plotIdA, plotIdB, options)` — fill between two plot series.

Colors are hex strings (e.g. `"#2196F3"`). Line style: `"solid"`, `"dotted"`, `"dashed"`.

## Manifest

The engine parses the script once to build a **manifest**: it runs the top-level declaration and `input.*()` calls to get `id`, `name`, `overlay`, `category`, and the list of parameters. The same script is then executed over each visible bar range to produce plot commands. The script file name (without `.js`) is used as the indicator `id`.

## Adding a custom script

1. Create a `.js` file (e.g. `my_indicator.js`).
2. Put it in the user scripts directory (see paths above), or in `assets/scripts/` for bundling.
3. Restart the app or use the indicator manager to refresh; the script should appear in the list and be loadable on a chart.

## Caching

Compiled bytecode is cached under `$KAIROS_DATA_PATH/cache/bytecode` (or the platform data dir) to speed up subsequent loads. Clear that directory if a script fails to update after edits.
