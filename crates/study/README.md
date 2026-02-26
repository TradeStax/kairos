# kairos-study

Technical studies and indicators for Kairos charts. Provides a trait-based
computation system that transforms market data (candles, trades) into abstract
render primitives (lines, bars, profiles, footprints, markers).

## Architecture

```
src/
├── lib.rs              Root re-exports and color constants
├── prelude.rs          Glob-import convenience module
├── error.rs            StudyError enum with severity classification
├── config/             Parameter definition, validation, and storage
│   ├── display.rs      DisplayFormat, Visibility (conditional UI logic)
│   ├── parameter.rs    ParameterDef, ParameterKind, ParameterTab
│   ├── store.rs        StudyConfig (HashMap-backed runtime config)
│   └── value.rs        ParameterValue, LineStyleValue
├── core/               Study trait and input/metadata types
│   ├── study.rs        Study trait (compute, output, reset, clone_study)
│   ├── input.rs        StudyInput (candles, trades, basis, tick_size)
│   └── metadata.rs     StudyCategory, StudyPlacement enums
├── output/             Abstract render primitives produced by studies
│   ├── series.rs       LineSeries, BarSeries, HistogramBar, PriceLevel
│   ├── markers.rs      TradeMarker, MarkerData, MarkerRenderConfig
│   ├── footprint/      Footprint chart data and configuration enums
│   │   ├── data.rs     FootprintData, FootprintCandle, FootprintLevel
│   │   ├── render.rs   CandleRenderConfig, mode/data-type/position enums
│   │   └── scaling.rs  FootprintScaling strategies (sqrt, log, hybrid)
│   └── profile/        Volume profile output and VBP configuration
│       ├── types.rs    ProfileLevel, ProfileSide, VolumeNode, ExtendDirection
│       └── vbp.rs      VBP-specific configs (POC, Value Area, Nodes, VWAP)
├── studies/            Built-in study implementations (16 total)
│   ├── registry.rs     register_built_ins() factory registration
│   ├── trend/          SMA, EMA, VWAP
│   ├── momentum/       RSI, MACD, Stochastic
│   ├── volume/         Volume, Delta, CVD, OBV
│   ├── volatility/     ATR, Bollinger Bands
│   └── orderflow/      Footprint, VBP, Big Trades, Imbalance
└── util/               Shared helpers
    ├── candle.rs       source_value(), candle_key()
    └── math.rs         mean(), variance(), standard_deviation()
```

## Usage

```rust
use kairos_study::{StudyRegistry, StudyInput, StudyOutput};

// Create registry with all built-in studies
let registry = StudyRegistry::new();

// Instantiate a study by ID
let mut sma = registry.create("sma").unwrap();

// Compute with market data
sma.compute(&StudyInput {
    candles: &candles,
    trades: None,
    basis: ChartBasis::Time(Timeframe::M1),
    tick_size: Price::from_f32(0.25),
    visible_range: None,
}).unwrap();

// Read output for rendering
match sma.output() {
    StudyOutput::Lines(series) => { /* draw lines */ }
    _ => {}
}
```

## Study trait

All studies implement `Study` which defines:

- **`compute(input)`** — Full recomputation from candle/trade data.
- **`append_trades(new_trades, input)`** — Incremental update (optional override).
- **`output()`** — Returns the last computed `StudyOutput`.
- **`reset()`** — Clears computed state.
- **`parameters()`** — Returns `ParameterDef` slice for the settings UI.
- **`set_parameter(key, value)`** — Validates and applies a config change.
- **`clone_study()`** — Heap-clone for `dyn Study` trait objects.

## Output types

`StudyOutput` is an enum of render primitives:

| Variant | Used by | Description |
|---------|---------|-------------|
| `Lines` | SMA, EMA, VWAP, RSI, Stochastic | Connected point series |
| `Band` | Bollinger | Upper/lower with optional fill |
| `Bars` | Volume, Delta | Colored bar chart |
| `Histogram` | MACD | Positive/negative bars |
| `Levels` | Imbalance | Horizontal price levels |
| `Profile` | VBP | Volume distribution at price levels |
| `Footprint` | Footprint | Per-candle trade data (replaces candles) |
| `Markers` | Big Trades | Sized/colored trade bubbles |
| `Composite` | RSI, MACD | Multiple outputs combined |

## Dependencies

- **`kairos-data`** — Market domain types (`Candle`, `Trade`, `Price`, `ChartBasis`)
- **`serde`** — Serialization for config persistence
- **`chrono`** — Date boundaries for CVD daily reset
- **`thiserror`** — Error derivation
