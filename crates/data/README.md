# kairos-data

Data infrastructure for the Kairos charting platform. Domain types, exchange adapters,
per-day caching, and the `DataEngine` facade. No GUI dependencies.

## Modules

| Module | Purpose |
|--------|---------|
| `domain` | Pure value objects and entities (no I/O, no async) |
| `adapter` | Exchange adapters: Databento (historical), Rithmic (real-time) |
| `aggregation` | Trade-to-candle aggregation (time-based and tick-based) |
| `cache` | Per-day bincode+zstd file storage with atomic writes |
| `connection` | Connection configuration and lifecycle management |
| `engine` | `DataEngine` facade: routes requests, manages adapters, emits events |
| `stream` | Stream subscription types (serializable and runtime-resolved) |
| `event` | `DataEvent` enum delivered via mpsc channel |
| `error` | Error hierarchy with `AppError` trait |
| `util` | Formatting, math, time, serde helpers |

## Key types

| Type | Description |
|------|-------------|
| `Price` | Fixed-point i64 with 10^-8 precision. Never use floats for price comparison. |
| `Trade` | Single execution: timestamp, price, quantity, side |
| `Candle` | OHLCV bar with buy/sell volume split |
| `Depth` | Order book snapshot as `BTreeMap<i64, f32>` |
| `FuturesTicker` | Inline 28-byte symbol with venue and expiration parsing |
| `FuturesTickerInfo` | Ticker with tick size, min quantity, contract size |
| `DataEngine` | Primary API facade for all market data operations |
| `DataEvent` | Events emitted by the engine via mpsc channel |
| `DataIndex` | Tracks available tickers, schemas, dates, and feed contributions |
| `ConnectionManager` | Stores connections, resolves best source for a ticker |

## Feature flags

| Flag | Default | Description |
|------|---------|-------------|
| `databento` | yes | Databento adapter for CME Globex historical data |
| `rithmic` | yes | Rithmic adapter for CME real-time streaming |
| `heatmap` | no | Depth snapshots and heatmap chart types |

## Architecture

```
domain  (pure types, no I/O)
  ↑
cache   (per-day bincode+zstd storage)
  ↑
adapter (Databento, Rithmic — feature-gated)
  ↑
engine  (DataEngine facade — routes requests, emits DataEvent)
```

## Price convention

All prices use the `Price` type (i64 with 10^-8 precision). Never use `f64` for price
storage or equality comparison. Convert with `Price::from_f32()` / `price.to_f64()`.
