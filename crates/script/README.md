# Kairos Script — JavaScript Studies

The `kairos-script` crate runs custom study scripts written in JavaScript (QuickJS). Scripts are discovered from:

- **Bundled**: `assets/studies/` (relative to the executable or current directory)
- **User**: `$KAIROS_DATA_PATH/studies` or platform data dir `kairos/studies` (e.g. `~/.local/share/kairos/studies` on Linux)

## Script structure

1. **Declaration** — Call `indicator(name, options)` once at top level.
2. **Inputs** — Use `input.*()` to define parameters (shown in the study settings UI).
3. **Logic** — Use `ta.*` helpers and your own logic to compute series.
4. **Output** — Call `plot()`, `plotBar()`, `plotHistogram()`, `marker()`, `hline()`, `fill()`, `plotProfile()`, `plotFootprint()`, etc.

### Example (SMA)

```javascript
indicator("Simple Moving Average", { overlay: true, category: "trend" });

const period = input.int("Period", 20, { min: 2, max: 500 });
const source = input.source("Source", close);
const color = input.color("Color", "#2196F3");
const width = input.float("Width", 1.5, { min: 0.5, max: 5.0, step: 0.5 });
const style = input.lineStyle("Style", "solid");

const smaValues = ta.sma(source, period);
plot(smaValues, "SMA", { color, lineWidth: width, style });

export {};
```

## API summary

### `indicator(name, options)`

- **name**: Display name in the UI.
- **options**: `{ overlay, category, placement }`.
  - **overlay**: `true` = draw on price chart; `false` = separate panel. Ignored if `placement` is set.
  - **placement**: One of `"overlay"`, `"panel"`, `"background"`, `"candle_replace"`.
  - **category**: One of `"trend"`, `"momentum"`, `"volume"`, `"volatility"`, `"orderflow"`.

### Render configuration

- `setMarkerRenderConfig({ shape, hollow, stdDev, minSize, maxSize, minOpacity, maxOpacity, showText, textSize, textColor })` — Configure marker rendering for trade marker studies.
- `setCandleRenderConfig({ defaultCellWidth, maxCellWidth, minCellWidth, cellHeightRatio, initialCandleWindow, autoscaleXCells })` — Configure cell layout for CandleReplace studies.

### Inputs

- `input.int(label, default, { min, max })`
- `input.float(label, default, { min, max, step })`
- `input.source(label, default)` — e.g. `close`, `open`, `high`, `low`, `hl2`, `hlc3`, `ohlc4`.
- `input.color(label, defaultHex)`
- `input.lineStyle(label, default)` — e.g. `"solid"`, `"dotted"`, `"dashed"`.
- `input.bool(label, default)`
- `input.choice(label, default, { options })` — dropdown with string options.

### Globals (injected each bar range)

- **OHLCV**: `open`, `high`, `low`, `close`, `volume`, `buyVolume`, `sellVolume` (arrays of numbers).
- **Time**: `time` (array of timestamps in ms).
- **Derived**: `hl2`, `hlc3`, `ohlc4`, `delta` (buyVolume − sellVolume per bar).
- **Trades**: `trades` (array of `{ time, price, quantity, isBuy }` objects).
- **Meta**: `barCount`, `tickSize`.

### Technical analysis (`ta.*`)

**Moving Averages**: `sma`, `ema`, `wma`, `vwma`, `rma`
**Oscillators**: `rsi`, `stoch`, `macd`
**Volatility**: `atr`, `bb`
**Volume**: `obv`, `cvd`, `cvdReset`, `vwap`, `vwapBands`
**Order Flow**: `buildProfile`, `buildFootprint`, `rollingPoc`, `valueArea`
**Utilities**: `crossover`, `crossunder`, `highest`, `lowest`, `change`, `roc`

### Plotting

- `plot(series, name, options)` — line; options: `color`, `lineWidth`, `style`.
- `plotBar(values, name, options)` — bar series.
- `plotBarColored(values, colors, name)` — bars with per-bar colors.
- `plotHistogram(values, name, options)` — histogram.
- `plotHistogramColored(values, colors, name)` — histogram with per-bar colors.
- `marker(time, price, options)` — single marker; options: `size`, `color`, `label`, `isBuy`.
- `hline(price, name, options)` — horizontal level.
- `fill(plotIdA, plotIdB, options)` — fill between two plot series.
- `plotProfile(profileData, options)` — volume profile; options: `side`.
- `plotFootprint(footprintData, options)` — footprint chart; options: `mode`, `dataType`, `scaling`, `candlePosition`.

Colors are hex strings (e.g. `"#2196F3"`). Line style: `"solid"`, `"dotted"`, `"dashed"`.

## Manifest

The engine parses the script once to build a **manifest**: it runs the top-level declaration and `input.*()` calls to get `id`, `name`, `placement`, `category`, and the list of parameters. The same script is then executed over each visible bar range to produce plot commands. The script file name (without `.js`) is used as the study `id`.

## Adding a custom script

1. Create a `.js` file (e.g. `my_study.js`).
2. Put it in the user scripts directory (see paths above), or in `assets/studies/` for bundling.
3. Restart the app or use the indicator manager to refresh; the script should appear in the list and be loadable on a chart.

## Caching

Compiled bytecode is cached under `$KAIROS_DATA_PATH/cache/bytecode` (or the platform data dir) to speed up subsequent loads. Clear that directory if a script fails to update after edits.
