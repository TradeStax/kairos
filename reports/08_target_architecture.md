# Target Architecture Design — Kairos

**Date**: 2026-02-20
**Author**: Target Architecture Agent
**Input**: Reports 01–07 (Architecture, Quality, Robustness, Performance, Consistency, Completeness, Synthesis)

---

## Executive Summary

This document defines the ideal target architecture for Kairos based on findings from seven independent audit reports covering 101 total findings (7 Critical, 23 High, 37 Medium, 34 Low). The redesign addresses three root causes that generate the majority of issues:

1. **Blurred crate boundaries** — GUI types in the data layer, adapter-specific methods in domain traits, parallel type systems across crates.
2. **Synchronous blocking on async runtime** — `block_on()` inside Iced's event loop and `spawn_blocking`, creating UI freezes and potential deadlocks.
3. **Organic growth without module decomposition** — God objects, 1,400-line files, duplicated constants, dead code hidden behind `#[allow(dead_code)]`.

The target architecture preserves the existing four-crate workspace structure (data, exchange, study, script) but enforces strict boundaries, eliminates type duplication, and introduces clear conventions for file organization, error handling, and async patterns.

---

## 1. Crate Structure

### 1.1 Workspace Layout

The workspace retains five crates. No new crates are introduced; no crates are merged. The `kairos-data` crate is cleaned of all I/O and GUI dependencies. The `kairos-exchange` crate wraps all third-party API types.

```
kairos/                           # Workspace root
├── Cargo.toml                    # Workspace manifest
├── src/                          # kairos (GUI binary crate)
├── data/                         # kairos-data (pure domain + state)
├── exchange/                     # kairos-exchange (adapters + repos)
├── study/                        # kairos-study (technical analysis)
└── script/                       # kairos-script (JS indicator engine)
```

### 1.2 Dependency Graph (Target)

```
                    kairos (GUI binary)
                   /     |      \       \
                  v      v       v       v
          kairos-data  kairos-exchange  kairos-script
               ^         |       |         |    |
               |         v       |         v    v
               +---------+      |    kairos-study
                                |         |
                                v         v
                            kairos-data  kairos-data
```

**Rules enforced in target state:**

- `kairos-data` has ZERO dependencies on `iced_core`, `open`, `dirs-next`, or `keyring`. It is a pure domain/state library.
- `kairos-exchange` depends on `kairos-data` for domain types. It does NOT re-export `kairos-data` domain types.
- `kairos-study` depends on `kairos-data` for `Price`, `Timestamp`, `Trade`, `Candle`.
- `kairos-script` depends on `kairos-data` and `kairos-study`.
- Only the `kairos` GUI crate depends on `iced`, `iced_core`, and platform I/O crates.

### 1.3 kairos-data Dependency Cleanup

**Remove from `data/Cargo.toml`:**

| Dependency | Reason | Replacement |
|------------|--------|-------------|
| `iced_core` | GUI framework leak into domain | Define local `Rgba(f32,f32,f32,f32)` and `ThemeId(String)` |
| `open` | Platform I/O (opens file browser) | Move `open_data_folder()` to GUI crate |
| `dirs-next` | Platform I/O (data directory) | Move `data_path()` to GUI crate or accept path as constructor arg |
| `keyring` | OS credential store I/O | Move `SecretsManager` to GUI crate |
| `base64` | Used only by `SecretsManager` | Moves with `keyring` |

**Target `data/Cargo.toml` dependencies:**

```toml
[dependencies]
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
rustc-hash.workspace = true
uuid.workspace = true
enum-map.workspace = true
log.workspace = true
thiserror.workspace = true
async-trait = "0.1"
tokio = { version = "1", features = ["sync", "time"] }
```

---

## 2. Module Boundaries

### 2.1 kairos-data — Target Module Tree

```
data/src/
├── lib.rs                          # Thin: module declarations + focused re-exports
├── error.rs                        # DataError enum (moved from lib.rs)
│
├── domain/                         # Pure business logic, zero I/O
│   ├── mod.rs                      # Re-exports
│   ├── types.rs                    # Price, Volume, Quantity, Timestamp, DateRange, TimeRange
│   ├── side.rs                     # TradeSide { Buy, Sell }, BookSide { Bid, Ask } (split from Side)
│   ├── entities.rs                 # Trade, Candle, DepthSnapshot, MarketData
│   ├── aggregation.rs              # Trade-to-candle aggregation
│   ├── error.rs                    # AppError trait, ErrorSeverity enum
│   ├── futures.rs                  # FuturesTicker, FuturesTickerInfo, Timeframe, ContractSpec
│   ├── options.rs                  # OptionContract, OptionChain, OptionSnapshot
│   ├── gex.rs                      # GEX domain types
│   ├── chart/                      # Chart domain (split from chart.rs 723 lines)
│   │   ├── mod.rs                  # Re-exports
│   │   ├── config.rs               # ChartConfig, ChartBasis, ChartType
│   │   ├── data.rs                 # ChartData, DataSegment, DataGap, MergeResult
│   │   ├── view.rs                 # ViewConfig, Autoscale, LoadingStatus
│   │   ├── kline.rs                # KlineDataPoint, KlineTrades, FootprintMode/Type
│   │   └── heatmap.rs              # HeatmapIndicator, CoalesceKind, HeatmapStudy
│   └── panel/                      # Panel-specific domain types
│       ├── mod.rs
│       ├── depth_grouping.rs
│       ├── trade_aggregator.rs
│       └── chase_tracker.rs
│
├── repository/                     # Async trait definitions ONLY
│   ├── mod.rs                      # Re-exports
│   └── traits.rs                   # TradeRepository, DepthRepository, Option*Repository
│                                   # NO Databento-specific methods
│
├── services/                       # Business orchestration
│   ├── mod.rs
│   ├── market_data.rs              # MarketDataService
│   ├── options_data.rs             # OptionsDataService
│   ├── gex_calculator.rs           # GexCalculationService
│   ├── cache_manager.rs            # CacheManagerService
│   ├── feed_merger.rs              # FeedMerger
│   └── replay_engine.rs            # ReplayEngine
│
├── state/                          # Serializable application state
│   ├── mod.rs
│   ├── app.rs                      # AppState
│   ├── chart.rs                    # ChartState (in-memory)
│   ├── layout.rs                   # Layout, Dashboard, LayoutManager
│   ├── pane.rs                     # Pane configuration
│   ├── registry.rs                 # DownloadedTickersRegistry
│   ├── replay.rs                   # ReplayState
│   └── persistence.rs              # Versioned serialization & migrations
│
├── config/                         # Configuration types
│   ├── mod.rs                      # ScaleFactor, MIN_SCALE, MAX_SCALE
│   ├── theme.rs                    # ThemeId(String), Rgba — NO iced_core
│   ├── timezone.rs                 # UserTimezone
│   ├── sidebar.rs                  # Sidebar config
│   └── panel.rs                    # Panel config
│
├── drawing/                        # Drawing entity types
│   ├── mod.rs
│   └── types.rs                    # DrawingId, DrawingStyle, SerializableColor (local Rgba)
│
├── feed/                           # Data feed model
│   ├── mod.rs
│   └── types.rs                    # FeedId, FeedConfig, FeedKind, DataFeedManager
│
├── products.rs                     # CANONICAL FUTURES_PRODUCTS constant (single source of truth)
│
└── util/                           # Utilities
    ├── mod.rs
    ├── formatting.rs
    ├── math.rs
    ├── time.rs
    └── logging.rs
```

**What belongs in kairos-data:**
- All domain value objects and entities
- Business rules (aggregation, GEX calculation, feed merging)
- Repository trait definitions (abstract, no adapter specifics)
- Serializable state and persistence
- Configuration types (using local color types, not iced_core)

**What does NOT belong in kairos-data:**
- GUI framework types (`iced_core::Color`, `iced_core::Theme`)
- Platform I/O (`open`, `dirs-next`, `keyring`)
- Adapter-specific methods (Databento cache/cost operations)
- `open_data_folder()`, `data_path()` (platform filesystem operations)
- `SecretsManager` (OS keyring access)

### 2.2 kairos-exchange — Target Module Tree

```
exchange/src/
├── lib.rs                          # Module declarations + focused re-exports
│                                   # NO type definitions, NO PushFrequency
│                                   # NO re-exports of kairos-data domain types
│                                   # NO re-exports of databento/rithmic-rs types
├── error.rs                        # Error enum with From<AdapterError>
├── types.rs                        # Wire-format types: RawTrade, RawKline, RawDepth
│                                   # Renamed from Trade/Kline/Depth to avoid confusion
│                                   # Single From<RawTrade> for data::Trade conversion
├── util.rs                         # exchange::Price (delegating to data::Price)
│                                   # PriceStep, Power10 types
│
├── adapter/
│   ├── mod.rs
│   ├── event.rs                    # Event enum
│   ├── stream.rs                   # StreamKind, PushFrequency (moved from lib.rs)
│   ├── error.rs                    # AdapterError
│   ├── databento/
│   │   ├── mod.rs                  # DatabentoAdapter facade
│   │   ├── client.rs               # API client wrapper
│   │   ├── cache.rs                # Cache management
│   │   ├── decoder.rs              # .dbn.zst decoding
│   │   ├── mapper.rs               # Wire → domain mapping
│   │   └── fetcher/                # Split from 1,411-line monolith
│   │       ├── mod.rs              # HistoricalDataManager (re-exports)
│   │       ├── manager.rs          # Lifecycle, orchestration
│   │       ├── gaps.rs             # Date range gap detection
│   │       ├── cost.rs             # API cost estimation
│   │       └── download.rs         # Concurrent download
│   ├── rithmic/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── mapper.rs
│   │   └── streaming.rs
│   └── massive/
│       ├── mod.rs
│       ├── fetcher.rs
│       └── mapper.rs
│
├── repository/
│   ├── mod.rs
│   ├── databento/
│   │   ├── mod.rs
│   │   ├── trades.rs               # DatabentoTradeRepository
│   │   └── depth.rs                # DatabentoDepthRepository
│   ├── rithmic/
│   │   ├── mod.rs
│   │   ├── trades.rs
│   │   └── depth.rs
│   └── massive/
│       ├── mod.rs
│       ├── chains.rs
│       ├── contracts.rs
│       └── snapshots.rs
│
└── ext/                            # Extension traits for provider-specific ops
    ├── mod.rs
    └── databento.rs                # DatabentoTradeExt: TradeRepository
                                    # check_cache_coverage, prefetch, get_cost, list_cached
```

**What belongs in kairos-exchange:**
- Wire-format types for each adapter (renamed to `RawTrade`, `RawKline`, `RawDepth`)
- Adapter implementations (API clients, streaming, caching)
- Repository trait implementations
- Extension traits for provider-specific capabilities
- `From<RawX> for data::X` conversions (single boundary)

**What does NOT belong in kairos-exchange:**
- Re-exports of `kairos-data` domain types (GUI should import from `data` directly)
- Re-exports of third-party types (`databento::dbn::Schema`, `rithmic_rs::RithmicEnv`)
- Domain type definitions (all canonical types live in `data`)

### 2.3 kairos-study — Target Module Tree (Minimal Changes)

```
study/src/
├── lib.rs                          # Re-exports
├── traits.rs                       # Study trait (compute returns Result)
├── output.rs                       # StudyOutput — prices as data::Price, not f64
├── config.rs                       # ParameterDef, ParameterValue, StudyConfig
├── error.rs                        # StudyError
├── registry.rs                     # StudyRegistry factory
├── util.rs                         # Shared computation helpers
│
├── volume/                         # Volume, Delta, CVD, OBV
├── trend/                          # SMA, EMA, VWAP
├── momentum/                       # RSI, MACD, Stochastic
├── volatility/                     # ATR, Bollinger Bands
└── orderflow/                      # Volume Profile, POC, Value Area, Imbalance, BigTrades
```

The study crate has the cleanest structure (Report 01, Finding 2.12). Only two changes needed:
1. `Study::compute()` returns `Result<(), StudyError>` instead of `()`
2. Output types use `data::Price` instead of raw `f64` for price fields

### 2.4 kairos (GUI crate) — Target Module Tree

```
src/
├── main.rs
├── error.rs                        # InternalError — implements AppError trait
├── window.rs                       # Multi-window management
├── layout.rs                       # Layout & Dashboard serialization
├── logger.rs                       # Async file logging
├── platform.rs                     # data_path(), open_data_folder() (moved from data crate)
├── secrets.rs                      # SecretsManager (moved from data crate)
│
├── app/
│   ├── mod.rs                      # Kairos struct (fields only) + new() + theme/title/scale
│   ├── globals.rs                  # OnceLock statics: DOWNLOAD_PROGRESS, RITHMIC_EVENTS, etc.
│   ├── messages.rs                 # Message, ChartMessage, DownloadMessage, OptionsMessage
│   ├── services.rs                 # Service initialization
│   ├── subscriptions.rs            # Subscription building
│   ├── ticker_registry.rs          # build_tickers_info() — reads from data::FUTURES_PRODUCTS
│   ├── view.rs                     # Kairos::view() — top-level view dispatch
│   ├── sidebar_view.rs             # view_with_modal() — all sidebar modal rendering
│   └── update/
│       ├── mod.rs                  # Kairos::update() — dispatch only
│       ├── chart.rs                # NO block_on — uses Task::perform
│       ├── download.rs
│       ├── feeds.rs
│       ├── navigation.rs
│       ├── options.rs
│       ├── preferences.rs
│       └── replay.rs               # NO block_on — uses Task::perform
│
├── chart/
│   ├── mod.rs                      # Re-exports only
│   ├── messages.rs                 # Message + Action enums
│   ├── update.rs                   # Generic update<T: Chart>()
│   ├── view.rs                     # Generic view<T>()
│   ├── volume_bar.rs               # draw_volume_bar() with VolumeBarSpec struct
│   ├── core/
│   │   ├── mod.rs
│   │   ├── traits.rs               # Chart, PlotConstants, DrawingCapable traits
│   │   ├── view_state.rs
│   │   ├── caches.rs
│   │   └── interaction/
│   ├── candlestick/
│   │   ├── mod.rs                  # KlineChart struct
│   │   ├── render.rs               # Rendering (binary search for visible candles)
│   │   └── footprint.rs            # TradeGroup, Footprint types + footprint rendering
│   ├── heatmap/
│   │   ├── mod.rs                  # HeatmapChart struct
│   │   ├── render.rs               # Rendering (precomputed volume profile)
│   │   └── data.rs                 # HeatmapData, DepthRun (BTreeMap for grouped trades)
│   ├── comparison/
│   │   ├── mod.rs                  # ComparisonChart struct
│   │   ├── render.rs               # Rendering (binary search for visible points)
│   │   └── types.rs                # Series (uses FuturesTickerInfo, not TickerInfo)
│   ├── study_renderer/             # StudyOutput → canvas draw calls
│   │   ├── mod.rs
│   │   ├── line.rs
│   │   ├── band.rs
│   │   ├── bar.rs
│   │   ├── histogram.rs
│   │   ├── profile.rs
│   │   └── markers.rs
│   ├── overlay/                    # Crosshair, ruler, last price, gap markers
│   ├── drawing/                    # Drawing tools with persistence
│   ├── scale/                      # Axis scaling
│   └── perf/                       # LOD rendering (rename to lod/)
│   # DELETED: src/chart/study/     # Dead code removed entirely
│
├── components/                     # NO #![allow(dead_code)] — unused items deleted
│   ├── mod.rs
│   ├── display/
│   ├── input/
│   ├── layout/
│   ├── overlay/
│   ├── form/
│   └── primitives/
│
├── modals/
│   ├── mod.rs                      # Single dashboard_modal function (delete old wrapper)
│   ├── pane/
│   │   ├── mod.rs
│   │   ├── tickers.rs
│   │   ├── calendar.rs
│   │   ├── indicator_manager.rs
│   │   ├── stream.rs
│   │   └── settings/
│   ├── connections/
│   ├── data_feeds/
│   ├── download/
│   ├── drawing_tools/
│   ├── layout/
│   ├── replay/                     # Split 1,009-line mod.rs into sub-modules
│   │   ├── mod.rs                  # ReplayManager struct
│   │   ├── view.rs                 # View rendering
│   │   ├── controller.rs           # Floating controller
│   │   └── messages.rs             # Message enum
│   └── theme/
│
├── screen/
│   └── dashboard/
│       ├── mod.rs
│       ├── sidebar.rs
│       ├── pane/
│       │   ├── mod.rs
│       │   ├── content.rs
│       │   ├── types.rs
│       │   ├── lifecycle.rs
│       │   ├── update.rs
│       │   └── view/
│       ├── panel/
│       │   ├── mod.rs
│       │   ├── ladder/             # Split from 1,283-line file
│       │   │   ├── mod.rs          # Ladder struct
│       │   │   ├── state.rs        # GroupedDepth, TradeStore, animation
│       │   │   └── render.rs       # Canvas rendering
│       │   └── timeandsales.rs
│       ├── layout/
│       └── loading/
│
└── style/
    ├── mod.rs
    ├── tokens.rs
    ├── palette.rs
    ├── button.rs
    ├── container.rs
    ├── canvas.rs
    └── widget.rs
```

---

## 3. Type System Design

### 3.1 Core Value Objects

All canonical types live in `data/src/domain/types.rs` and `data/src/domain/side.rs`:

```rust
// data/src/domain/types.rs — THE source of truth for value objects

/// Price with fixed precision (10^-8)
/// ALL prices in the system use this type.
/// Raw f32/f64 is only permitted at:
///   - Canvas rendering coordinates (clearly documented)
///   - Wire-format deserialization boundaries (in exchange crate)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Price { units: i64 }  // 10^-8 precision, private field

/// Timestamp in milliseconds since epoch
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

/// Volume (quantity traded) — f64 for fractional lots
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Volume(pub f64);

/// Quantity (position/order size) — f64 for fractional lots
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Quantity(pub f64);

/// Date range (start inclusive, end inclusive)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateRange { pub start: NaiveDate, pub end: NaiveDate }

/// Time range in milliseconds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange { pub start: Timestamp, pub end: Timestamp }
```

```rust
// data/src/domain/side.rs — Split Side into two semantically distinct enums

/// Trade execution side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TradeSide { Buy, Sell }

/// Order book side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BookSide { Bid, Ask }

impl TradeSide {
    pub fn to_book_side(self) -> BookSide {
        match self { TradeSide::Buy => BookSide::Bid, TradeSide::Sell => BookSide::Ask }
    }
}

impl BookSide {
    pub fn to_trade_side(self) -> TradeSide {
        match self { BookSide::Bid => TradeSide::Buy, BookSide::Ask => TradeSide::Sell }
    }
}
```

### 3.2 Price Type Unification

**Current state:** Two `Price` structs — `data::Price` (private `units`, unchecked arithmetic) and `exchange::util::Price` (public `units`, checked arithmetic with panicking `expect`). Plus `FuturesTickerInfo.tick_size: f32`.

**Target state:** Single canonical `data::Price` with merged capabilities:

```rust
// data/src/domain/types.rs — unified Price

impl Price {
    pub const PRECISION: i64 = 100_000_000; // 10^8
    pub const PRICE_SCALE: i32 = 8;

    // Construction
    pub fn from_units(units: i64) -> Self;
    pub fn from_f32(value: f32) -> Self;      // Lossy
    pub fn from_f64(value: f64) -> Self;
    pub const fn zero() -> Self;

    // Conversion
    pub fn units(self) -> i64;
    pub fn to_f32(self) -> f32;               // Lossy
    pub fn to_f64(self) -> f64;

    // Arithmetic (saturating by default — NEVER panics in production)
    pub fn saturating_add(self, other: Self) -> Self;
    pub fn saturating_sub(self, other: Self) -> Self;
    pub fn checked_add(self, other: Self) -> Option<Self>;
    pub fn checked_sub(self, other: Self) -> Option<Self>;

    // Rounding
    pub fn round_to_tick(self, tick_size: Price) -> Self;
    pub fn round_to_side_step(self, is_sell_or_bid: bool, step: Price) -> Self;
    pub fn add_steps(self, steps: i64, step: Price) -> Self;  // saturating
    pub fn steps_between_inclusive(low: Price, high: Price, step: Price) -> Option<usize>;

    // Formatting (from exchange::Price)
    pub fn fmt_with_precision<W: fmt::Write>(self, decimals: u32, out: &mut W) -> fmt::Result;
}

// Operator overloads use SATURATING arithmetic (no panics)
impl Add for Price {
    fn add(self, other: Self) -> Self { self.saturating_add(other) }
}
impl Sub for Price {
    fn sub(self, other: Self) -> Self { self.saturating_sub(other) }
}
```

**`exchange::util.rs` target state:** Re-exports `data::Price` and provides only exchange-specific helpers:

```rust
// exchange/src/util.rs
pub use kairos_data::Price;  // Single canonical type

// Exchange-specific: PriceStep for tick-size-based operations
pub struct PriceStep { pub units: i64 }

// Exchange-specific: Power10 for wire format precision
pub struct Power10<const MIN: i8, const MAX: i8> { pub power: i8 }

// Conversion helpers for wire format
pub fn ms_to_datetime(ms: u64) -> Option<DateTime<Utc>>;
```

**`FuturesTickerInfo` target state:**

```rust
// data/src/domain/futures.rs
pub struct FuturesTickerInfo {
    pub ticker: FuturesTicker,
    pub tick_size: Price,       // Changed from f32 to Price
    pub min_qty: f32,
    pub contract_size: f32,
}
```

The `exchange::TickerInfo` type is **deleted**. `FuturesTickerInfo` is the single type.

### 3.3 Wire-Format Types (Exchange Layer)

Rename exchange types to clarify they are wire-format intermediates:

```rust
// exchange/src/types.rs

/// Raw trade from exchange wire format (Databento MBP, Rithmic)
/// Converted to data::Trade at the adapter boundary via From.
pub struct RawTrade {
    pub time: u64,       // millis
    pub price: f32,      // wire precision
    pub qty: f32,
    pub side: RawTradeSide,
}

pub struct RawKline {
    pub time: u64,
    pub open: f32, pub high: f32, pub low: f32, pub close: f32,
    pub buy_volume: f32, pub sell_volume: f32,
}

pub struct RawDepth {
    pub time: u64,
    pub bids: BTreeMap<i64, f32>,  // price_units -> qty
    pub asks: BTreeMap<i64, f32>,
}

// Single conversion boundary
impl From<RawTrade> for data::Trade {
    fn from(raw: RawTrade) -> Self {
        data::Trade {
            time: Timestamp::from_millis(raw.time),
            price: Price::from_f32(raw.price),
            qty: Volume(raw.qty as f64),
            side: match raw.side {
                RawTradeSide::Buy => TradeSide::Buy,
                RawTradeSide::Sell => TradeSide::Sell,
            },
        }
    }
}
```

### 3.4 Color Type (Data Layer)

Replace all `iced_core::Color` usage in the data crate:

```rust
// data/src/drawing/types.rs (or data/src/config/theme.rs)

/// GUI-independent RGBA color for serialization
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// Theme identifier — references a named theme, not the theme object itself
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeId(pub String);
```

The GUI crate provides `impl From<Rgba> for iced::Color` and `impl From<iced::Color> for Rgba` at the view boundary.

### 3.5 Error Type Hierarchy

```
                  AppError (trait, data crate)
                 /          |           \
        DataError      RepositoryError   AggregationError
        (data)         (data)            (data)
                           |
                    ┌──────┴──────┐
              exchange::Error   AdapterError
              (From<AdapterError>)
                           |
                    InternalError (GUI crate)
                    (implements AppError)
```

**Target changes:**

1. `InternalError` implements `AppError` trait (currently it does not):

```rust
// src/error.rs
#[derive(Error, Debug, Clone)]
pub enum InternalError {
    #[error("Chart error: {message}")]
    Chart { message: String, severity: ErrorSeverity, retriable: bool },
    #[error("Data error: {message}")]
    Data { message: String, severity: ErrorSeverity, retriable: bool },
    #[error("Rendering error: {message}")]
    Rendering { message: String, severity: ErrorSeverity, retriable: bool },
}

impl AppError for InternalError {
    fn user_message(&self) -> String { /* match arm */ }
    fn is_retriable(&self) -> bool { /* match arm */ }
    fn severity(&self) -> ErrorSeverity { /* match arm */ }
}
```

2. `From<AdapterError> for exchange::Error` is implemented (currently missing).

3. Message enums preserve error structure:

```rust
// src/app/messages.rs
pub struct CloneableError {
    pub message: String,
    pub severity: ErrorSeverity,
    pub retriable: bool,
}

// Used in message variants instead of Result<T, String>
ChartDataLoaded {
    layout_id: uuid::Uuid,
    pane_id: uuid::Uuid,
    result: Result<data::ChartData, CloneableError>,
},
```

### 3.6 Serialization Strategy

- All persisted state uses `serde` with JSON format.
- State versioning uses `StateVersion` newtype consistently (not raw `u32`).
- State writes are **atomic**: write to `app-state.json.tmp`, then `fs::rename`.
- `FuturesTicker` serialization uses `self.venue.to_string()` not hardcoded `"CMEGlobex"`.
- `Timeframe` Display and serde produce identical strings (enforced by test).
- All `Rgba` color values replace `iced_core::Color` in persisted state.
- Migration v2→v3 converts `iced_core::Color` fields to `Rgba` format.

---

## 4. Trait Architecture

### 4.1 Chart Traits

```rust
// src/chart/core/traits.rs

/// Core trait for all chart types
pub trait Chart: PlotConstants + canvas::Program<Message> {
    fn state(&self) -> &ViewState;
    fn mut_state(&mut self) -> &mut ViewState;
    fn invalidate_all(&mut self);
    fn invalidate_crosshair(&mut self);
    fn interval_keys(&self) -> Option<Vec<u64>>;
    fn autoscaled_coords(&self) -> Vector;
    fn supports_fit_autoscaling(&self) -> bool;
    fn is_empty(&self) -> bool;
}

/// Sizing constants for chart zoom limits
pub trait PlotConstants {
    fn max_cell_width(&self) -> f32;
    fn min_cell_width(&self) -> f32;
    fn max_cell_height(&self) -> f32;
    fn min_cell_height(&self) -> f32;
    fn default_cell_width(&self) -> f32;
}

/// Optional capability: drawing support
/// Only KlineChart implements this.
pub trait DrawingCapable: Chart {
    fn active_drawing_tool(&self) -> DrawingTool;
    fn has_pending_drawing(&self) -> bool;
    fn hit_test_drawing(&self, screen_point: Point, bounds: Size) -> Option<DrawingId>;
    fn hit_test_drawing_handle(&self, screen_point: Point, bounds: Size)
        -> Option<(DrawingId, usize)>;
    fn has_drawing_selection(&self) -> bool;
    fn is_drawing_selected(&self, id: DrawingId) -> bool;
    fn has_clone_pending(&self) -> bool;
}
```

### 4.2 Repository Traits

```rust
// data/src/repository/traits.rs — CLEAN domain traits

#[async_trait]
pub trait TradeRepository: Send + Sync {
    async fn get_trades(&self, ticker: &FuturesTicker, range: &DateRange)
        -> RepositoryResult<Vec<Trade>>;
    async fn get_trades_with_progress(&self, ticker: &FuturesTicker, range: &DateRange,
        callback: Box<dyn Fn(LoadingStatus) + Send + Sync>) -> RepositoryResult<Vec<Trade>>;
    async fn has_trades(&self, ticker: &FuturesTicker, date: NaiveDate) -> RepositoryResult<bool>;
    async fn get_trades_for_date(&self, ticker: &FuturesTicker, date: NaiveDate)
        -> RepositoryResult<Vec<Trade>>;
    async fn store_trades(&self, ticker: &FuturesTicker, date: NaiveDate, trades: Vec<Trade>)
        -> RepositoryResult<()>;
    async fn find_gaps(&self, ticker: &FuturesTicker, range: &DateRange)
        -> RepositoryResult<Vec<DateRange>>;
    async fn stats(&self) -> RepositoryResult<RepositoryStats>;
    // NO Databento-specific methods
}

// exchange/src/ext/databento.rs — Provider-specific extension

#[async_trait]
pub trait DatabentoTradeExt: TradeRepository {
    async fn check_cache_coverage(&self, ticker: &FuturesTicker,
        schema: databento::dbn::Schema, range: &DateRange) -> RepositoryResult<CacheCoverageReport>;
    async fn prefetch_to_cache(&self, ticker: &FuturesTicker,
        schema: databento::dbn::Schema, range: &DateRange) -> RepositoryResult<usize>;
    async fn prefetch_with_progress(&self, ticker: &FuturesTicker,
        schema: databento::dbn::Schema, range: &DateRange,
        callback: Box<dyn Fn(usize, usize) + Send + Sync>) -> RepositoryResult<usize>;
    async fn get_actual_cost(&self, ticker: &FuturesTicker,
        schema: databento::dbn::Schema, range: &DateRange) -> RepositoryResult<f64>;
    async fn list_cached_symbols(&self) -> RepositoryResult<HashSet<String>>;
}
```

### 4.3 Study Trait

```rust
// study/src/traits.rs

pub trait Study: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn category(&self) -> StudyCategory;
    fn placement(&self) -> StudyPlacement;
    fn config(&self) -> &StudyConfig;
    fn set_parameter(&mut self, key: &str, value: ParameterValue) -> Result<(), StudyError>;
    fn output(&self) -> &StudyOutput;

    /// Compute study from input data. Returns Err on computation failure.
    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError>;  // Changed from ()

    /// Incremental update (optional, default calls full compute)
    fn update(&mut self, input: &StudyInput) -> Result<(), StudyError> {
        self.compute(input)
    }
}
```

### 4.4 Error Trait

```rust
// data/src/domain/error.rs — unchanged, this design is sound

pub trait AppError: std::error::Error {
    fn user_message(&self) -> String;
    fn is_retriable(&self) -> bool;
    fn severity(&self) -> ErrorSeverity;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Recoverable,
    Critical,
}
```

---

## 5. Dependency Graph (Ideal)

```
┌─────────────────────────────────────────────────────────────────┐
│                        kairos (GUI binary)                      │
│  ┌──────────────────┬─────────────────┬──────────────────────┐  │
│  │  app/            │  chart/         │  screen/dashboard/   │  │
│  │  ├─ messages     │  ├─ candlestick │  ├─ pane/            │  │
│  │  ├─ services     │  ├─ heatmap     │  ├─ panel/           │  │
│  │  ├─ update/      │  ├─ comparison  │  └─ sidebar          │  │
│  │  └─ view         │  └─ study_renderer │                   │  │
│  └──────────────────┴─────────────────┴──────────────────────┘  │
│  ┌──────────────┬─────────────────────┬──────────────────────┐  │
│  │  components/  │  modals/           │  style/              │  │
│  │  platform.rs  │  secrets.rs        │  window.rs           │  │
│  └──────────────┴─────────────────────┴──────────────────────┘  │
└──────────┬────────────┬────────────────────┬────────────────────┘
           │            │                    │
           v            v                    v
┌──────────────┐  ┌──────────────┐    ┌─────────────┐
│ kairos-data  │  │ kairos-      │    │ kairos-     │
│              │  │ exchange     │    │ script      │
│ domain/      │  │              │    │             │
│ ├─ types     │  │ adapter/     │    │ runtime/    │
│ ├─ entities  │  │ ├─ databento │    │ bridge/     │
│ ├─ chart/    │  │ ├─ rithmic   │    │ loader/     │
│ ├─ futures   │  │ └─ massive   │    └──────┬──────┘
│ ├─ options   │  │              │           │
│ ├─ gex       │  │ repository/  │           │
│ └─ panel/    │  │ ext/         │           │
│              │  │ types.rs     │           │
│ repository/  │  │ (RawTrade)   │           │
│ services/    │  │              │           │
│ state/       │  └───────┬──────┘           │
│ config/      │          │                  │
│ feed/        │          v                  v
│ drawing/     │  ┌──────────────┐    ┌─────────────┐
│ products.rs  │  │ kairos-data  │    │ kairos-     │
│ util/        │  │ (domain      │    │ study       │
│              │  │  types only) │    │             │
│ NO iced_core │  └──────────────┘    │ traits.rs   │
│ NO keyring   │                      │ output.rs   │
│ NO open      │                      │ registry.rs │
│ NO dirs-next │                      │ volume/     │
└──────────────┘                      │ trend/      │
       ^                              │ momentum/   │
       │                              │ volatility/ │
       │                              │ orderflow/  │
       │                              └──────┬──────┘
       │                                     │
       └─────────────────────────────────────┘
                  (kairos-study depends on kairos-data)
```

**Key invariants:**
- Arrows point DOWN = dependency direction
- `kairos-data` is at the bottom — no upward dependencies
- No circular dependencies
- Third-party types (databento, rithmic-rs) are fully encapsulated within `kairos-exchange`
- GUI types (iced, iced_core) are fully encapsulated within `kairos` (GUI crate)

---

## 6. File Naming & Organization Conventions

### 6.1 `mod.rs` vs Named Files

**Rule:** Use `mod.rs` only when the module has 2+ child files. Single-file modules use named files.

```
# CORRECT: directory with children
chart/candlestick/
├── mod.rs           # KlineChart struct, re-exports
├── render.rs        # Rendering logic
└── footprint.rs     # Footprint types and rendering

# CORRECT: single-file module
chart/volume_bar.rs  # draw_volume_bar function

# WRONG: directory with only mod.rs
secrets/
└── mod.rs           # 436 lines, no children
# SHOULD BE: secrets.rs (or moved to GUI crate)
```

### 6.2 File Size Limits

| Threshold | Action Required |
|-----------|----------------|
| < 600 lines | Acceptable |
| 600–800 lines | Review for split opportunities |
| 800–1000 lines | Must be split before next PR |
| > 1000 lines | Critical — split immediately |

### 6.3 Nesting Depth

Maximum module nesting: 4 levels from crate root.

```
# OK (4 levels):
src/screen/dashboard/pane/view/kline.rs

# Too deep (5+ levels):
src/screen/dashboard/pane/view/controls/toolbar.rs
# → Move to: src/screen/dashboard/pane/controls_toolbar.rs
```

### 6.4 `#[allow(dead_code)]` Policy

- **Module-level `#![allow(dead_code)]`**: NEVER permitted. Each unused item must be individually addressed (deleted or `#[allow]` with a tracking comment linking to an issue).
- **Item-level `#[allow(dead_code)]`**: Permitted only with a comment explaining WHY (e.g., `#[allow(dead_code)] // Used by kairos-script via reflection`).
- **Feature-gated dead code**: Use `#[cfg(feature = "options")]` instead of `#[allow(dead_code)]` for incomplete features.

### 6.5 Test Organization

- Unit tests: `#[cfg(test)] mod tests { ... }` at the bottom of the source file.
- Integration tests: `tests/` directory at the crate root.
- Test assertions: Use `assert_matches!` (std, stable since Rust 1.82) instead of `match { ... panic!() }`.
- Test data: Shared fixtures in `tests/fixtures/` directory.

### 6.6 Import Conventions

```rust
// Canonical import paths — GUI crate imports domain types from data, not exchange
use data::Price;               // CORRECT
use exchange::Price;           // WRONG — exchange should not re-export Price

use data::FuturesTicker;      // CORRECT
use exchange::FuturesTicker;  // WRONG — exchange should not re-export this

use exchange::DatabentoTradeRepository;  // CORRECT — repo impl lives in exchange
```

---

## 7. Migration Path

### Migration Phase 0: Foundation (No Breaking API Changes)

These changes are internal and do not alter the public API of any crate.

#### 7.0.1 Fix `active_dashboard().expect()` Crash Path

**What:** `src/app/state.rs` — `active_dashboard()` and `active_dashboard_mut()` call `.expect()`.
**Move to:** Return `Option<&Dashboard>` / `Option<&mut Dashboard>`.
**Breaking:** Internal to GUI crate only. Callers must handle `None` (show empty state or create default layout).
**Files touched:** `src/app/state.rs`, all callers in `src/app/update/`, `src/app/mod.rs`.

#### 7.0.2 Replace `block_on()` With `Task::perform`

**What:** 9+ `block_on()` calls in `src/app/update/replay.rs`, 2 in `src/app/update/chart.rs`, 2 in `src/app/services.rs`.
**Move to:** All async operations go through `Task::perform(async move { ... })`.
**Breaking:** Replay behavior may change subtly (operations become truly async). Manual testing required.
**Files touched:** `src/app/update/replay.rs`, `src/app/update/chart.rs`, `src/app/services.rs`.

#### 7.0.3 Make State Persistence Atomic

**What:** `data/src/state/persistence.rs` — `save_state` writes directly to `app-state.json`.
**Move to:** Write to `app-state.json.tmp`, then `fs::rename`.
**Breaking:** None (transparent improvement).
**Files touched:** `data/src/state/persistence.rs`.

### Migration Phase 1: Type System Unification

#### 7.1.1 Unify Price Types

**What:** `data::Price` and `exchange::util::Price` are separate types with `From` conversions.
**Move to:** `data::Price` is the single canonical type. `exchange::util` re-exports `data::Price` and keeps only `PriceStep` and `Power10`.
**Steps:**
1. Merge `exchange::Price` checked arithmetic into `data::Price` (as saturating).
2. Change `data::Price` operator overloads to use `saturating_add`/`saturating_sub`.
3. Delete `exchange::util::Price` struct definition.
4. Add `pub use kairos_data::Price;` to `exchange/src/util.rs`.
5. Update all `exchange::util::Price` usages to `data::Price`.
**Breaking:** `exchange::util::Price.units` was public; `data::Price.units` is private (use `.units()` method).
**Files touched:** ~50 files across all crates.

#### 7.1.2 Change `FuturesTickerInfo.tick_size` from `f32` to `Price`

**What:** `data/src/domain/futures.rs` — `tick_size: f32`.
**Move to:** `tick_size: Price`.
**Steps:**
1. Change field type.
2. Update `FuturesTickerInfo::new()` to accept `Price`.
3. Update `FUTURES_PRODUCTS` constant to store `Price` values.
4. Delete `min_ticksize()` conversion method (now identity).
**Files touched:** `data/src/domain/futures.rs`, `src/app/mod.rs` (build_tickers_info), all consumers.

#### 7.1.3 Delete `exchange::TickerInfo` — Unify to `FuturesTickerInfo`

**What:** `exchange/src/types.rs` defines `TickerInfo` as a near-duplicate of `FuturesTickerInfo`.
**Move to:** Delete `TickerInfo`. All code uses `FuturesTickerInfo`.
**Steps:**
1. Replace all `TickerInfo` usages in `Ladder`, `TimeAndSales`, `ComparisonChart` with `FuturesTickerInfo`.
2. Delete `src/chart/comparison/mod.rs` compatibility shim (`ticker_info_to_old_format`).
3. Delete `exchange/src/types.rs::TickerInfo`.
**Files touched:** `exchange/src/types.rs`, `src/screen/dashboard/panel/ladder.rs`, `src/screen/dashboard/panel/timeandsales.rs`, `src/chart/comparison/`.

#### 7.1.4 Split `Side` Into `TradeSide` + `BookSide`

**What:** `data/src/domain/types.rs` — `Side { Buy, Sell, Bid, Ask }`.
**Move to:** Two enums: `TradeSide { Buy, Sell }` and `BookSide { Bid, Ask }`.
**Steps:**
1. Create `data/src/domain/side.rs` with both enums.
2. Update `Trade.side` to `TradeSide`.
3. Update `DepthSnapshot` usage to `BookSide`.
4. Update all match arms across crates.
**Files touched:** Domain types, all chart rendering, exchange mappers.

#### 7.1.5 Rename Exchange Wire Types

**What:** `exchange::types::Trade`, `Kline`, `Depth` conflict semantically with domain types.
**Move to:** `RawTrade`, `RawKline`, `RawDepth`.
**Steps:**
1. Rename structs and update all usages within exchange crate.
2. Ensure single `From<RawTrade> for data::Trade` conversion.
**Files touched:** `exchange/src/types.rs`, all adapter/mapper files.

### Migration Phase 2: Architecture Cleanup

#### 7.2.1 Remove `iced_core` From `kairos-data`

**What:** `data/Cargo.toml` depends on `iced_core`. Used for `Theme(iced_core::Theme)` and `iced_core::Color` in pane state.
**Move to:**
1. Define `data::Rgba` and `data::ThemeId` as local types.
2. Replace `iced_core::Color` in `CandleStyle`, `ComparisonConfig`, `SerializableColor`.
3. Replace `Theme(iced_core::Theme)` with `ThemeId(String)`.
4. Write state migration v2→v3 for the serialization format change.
5. In GUI crate: `impl From<Rgba> for iced::Color` and `impl From<iced::Color> for Rgba`.
**Breaking:** Persisted state format changes (handled by migration).
**Files touched:** `data/Cargo.toml`, `data/src/config/theme.rs`, `data/src/state/pane.rs`, `data/src/drawing/types.rs`, `src/app/mod.rs`.

#### 7.2.2 Move Platform I/O Out of `kairos-data`

**What:** `data/src/lib.rs` — `data_path()`, `open_data_folder()` use `dirs-next` and `open`.
**Move to:** `src/platform.rs` in the GUI crate.
**Steps:**
1. Create `src/platform.rs` with `pub fn data_path()` and `pub fn open_data_folder()`.
2. Update all callers to use `crate::platform::data_path()`.
3. Remove `dirs-next` and `open` from `data/Cargo.toml`.
**Files touched:** `data/src/lib.rs`, `data/Cargo.toml`, all callers in `src/`.

#### 7.2.3 Move `SecretsManager` Out of `kairos-data`

**What:** `data/src/secrets/mod.rs` — 436 lines of OS keyring access.
**Move to:** `src/secrets.rs` in the GUI crate.
**Steps:**
1. Move `data/src/secrets/mod.rs` to `src/secrets.rs`.
2. Keep `ApiProvider` and `ApiKeyStatus` enums in `data/src/config/` (they are domain config).
3. Remove `keyring` and `base64` from `data/Cargo.toml`.
**Files touched:** `data/src/secrets/`, `data/Cargo.toml`, `src/app/services.rs`, `src/app/update/feeds.rs`.

#### 7.2.4 Break Up `src/app/mod.rs`

**What:** 1,063-line god module.
**Move to:** Five extracted files.
**Steps:**
1. Extract `DOWNLOAD_PROGRESS`, `RITHMIC_EVENTS`, `REPLAY_EVENTS`, `RITHMIC_SERVICE_RESULT` to `src/app/globals.rs`.
2. Extract `Message`, `ChartMessage`, `DownloadMessage`, `OptionsMessage` to `src/app/messages.rs`.
3. Extract `view_with_modal()` to `src/app/sidebar_view.rs`.
4. Extract `FUTURES_PRODUCTS` and `build_tickers_info()` to `src/app/ticker_registry.rs`.
5. Extract `view()` to `src/app/view.rs`.
6. `mod.rs` keeps only: `Kairos` struct definition, `new()`, `title()`, `theme()`, `scale_factor()`, `subscription()`.
**Target `mod.rs` size:** ~200 lines.
**Files touched:** `src/app/mod.rs` (split), all files importing from `app::`.

#### 7.2.5 Consolidate `FUTURES_PRODUCTS`

**What:** Three separate definitions with divergent data.
**Move to:** Single canonical `data/src/products.rs`.
**Steps:**
1. Create `data/src/products.rs` with a full `FuturesProduct` struct:
   ```rust
   pub struct FuturesProduct {
       pub symbol: &'static str,
       pub display_name: &'static str,
       pub tick_size: Price,
       pub min_qty: f32,
       pub contract_size: f32,
   }
   pub const FUTURES_PRODUCTS: &[FuturesProduct] = &[ ... ];
   ```
2. Delete `src/app/mod.rs::FUTURES_PRODUCTS`.
3. Delete `src/modals/download/mod.rs` duplicate.
4. Update `exchange/src/adapter/databento/mapper.rs` to use `data::FUTURES_PRODUCTS`.
**Files touched:** 3 files with definitions + all consumers.

#### 7.2.6 Extract Databento Methods From `TradeRepository`

**What:** `data/src/repository/traits.rs` — 5 `_databento` methods on `TradeRepository`.
**Move to:** `exchange/src/ext/databento.rs` — `DatabentoTradeExt` extension trait.
**Steps:**
1. Create `exchange/src/ext/databento.rs` with extension trait.
2. Remove `_databento` methods from `TradeRepository`.
3. `DatabentoTradeRepository` implements both `TradeRepository` and `DatabentoTradeExt`.
4. GUI code that needs Databento-specific ops imports from `exchange::ext::DatabentoTradeExt`.
**Files touched:** `data/src/repository/traits.rs`, `exchange/src/repository/databento/trades.rs`, callers in `src/`.

#### 7.2.7 Delete Dead Code

**What:** `src/chart/study/` (dead module), `src/chart/indicator/` (deleted files), options pipeline dead code.
**Steps:**
1. Move `TradeGroup` and `Footprint` type aliases from `src/chart/study/` to `src/chart/candlestick/footprint.rs`.
2. Delete `src/chart/study/` directory entirely.
3. Feature-gate options pipeline with `#[cfg(feature = "options")]`.
4. Remove all `#![allow(dead_code)]` from component modules; delete actually unused items.
**Files touched:** `src/chart/mod.rs`, `src/chart/study/`, `src/components/`, `src/app/mod.rs`.

#### 7.2.8 Split Large Files

| File | Lines | Split Into |
|------|-------|------------|
| `exchange/src/adapter/databento/fetcher.rs` | 1,411 | `fetcher/mod.rs`, `fetcher/manager.rs`, `fetcher/gaps.rs`, `fetcher/cost.rs`, `fetcher/download.rs` |
| `src/screen/dashboard/panel/ladder.rs` | 1,283 | `ladder/mod.rs`, `ladder/state.rs`, `ladder/render.rs` |
| `study/src/orderflow/big_trades.rs` | 1,229 | Keep (50% is tests); extract test helpers to `big_trades_tests.rs` |
| `src/modals/pane/indicator_manager.rs` | 1,114 | `indicator_manager/mod.rs`, `indicator_manager/view.rs`, `indicator_manager/state.rs` |
| `src/modals/replay/mod.rs` | 1,009 | `replay/mod.rs`, `replay/view.rs`, `replay/controller.rs`, `replay/messages.rs` |
| `data/src/domain/chart.rs` | 723 | `chart/mod.rs`, `chart/config.rs`, `chart/data.rs`, `chart/view.rs`, `chart/kline.rs`, `chart/heatmap.rs` |

### Migration Phase 3: Error Handling & Robustness

#### 7.3.1 Implement `AppError` on `InternalError`
#### 7.3.2 Add `From<AdapterError> for exchange::Error`
#### 7.3.3 Standardize Mutex Poison Recovery

Use `data::lock_or_recover()` (which already exists) everywhere. Replace all bare `.lock().unwrap()` with `lock_or_recover()`.

#### 7.3.4 Replace Production `assert!` With `debug_assert!`

`data/src/domain/aggregation.rs:126,276` — change `assert!(!trades.is_empty())` to `debug_assert!` and add an early return with `Err` at the call site.

#### 7.3.5 Replace `from_utf8().unwrap()` With `from_utf8_lossy()`

`data/src/domain/futures.rs:277,282,292` — use `from_utf8_lossy` or return `Result`.

### Migration Phase 4: Performance

#### 7.4.1 Heatmap: Compute `visible_data_count` Once Per Frame
#### 7.4.2 Candlestick: Binary Search for Visible Candles and Crosshair Lookup
#### 7.4.3 Heatmap: Precompute Volume Profile Outside Draw Closure
#### 7.4.4 Heatmap: Replace Vec Linear Scan in `add_trade` With BTreeMap
#### 7.4.5 Comparison: Replace Linear Scan With `partition_point`
#### 7.4.6 Aggregation: Move Sort Validation to `debug_assert!` Only

### Migration Phase 5: Feature Completion

#### 7.5.1 Wire Options Pipeline or Feature-Gate It
#### 7.5.2 Implement Link Group Ticker Switching
#### 7.5.3 Implement Replay JumpForward/JumpBackward
#### 7.5.4 Implement FocusWidget Effect
#### 7.5.5 Add Test Coverage for GUI Crate

### Migration Phase 6: Polish

#### 7.6.1 Replace "XX & Company" / "XXNCO" With `const APP_NAME`
#### 7.6.2 Rename `FLOWSURFACE_DATA_PATH` to `KAIROS_DATA_PATH`
#### 7.6.3 Update CLAUDE.md to Reflect Target Architecture
#### 7.6.4 Populate Root README.md
#### 7.6.5 Document JS Scripting System
#### 7.6.6 Stop Re-exporting Third-Party Types from `exchange/src/lib.rs`
#### 7.6.7 Stop Re-exporting Domain Types from `exchange/src/lib.rs`
#### 7.6.8 Reduce `data/src/lib.rs` Re-export Surface to ~20 Lines

---

## Appendix A: Summary of Type Unifications

| Current (Duplicated) | Target (Canonical) | Location |
|---|---|---|
| `data::Price` + `exchange::Price` | `data::Price` (saturating arithmetic) | `data/src/domain/types.rs` |
| `data::Side { Buy, Sell, Bid, Ask }` + `exchange::TradeSide { Buy, Sell }` | `data::TradeSide` + `data::BookSide` | `data/src/domain/side.rs` |
| `data::FuturesTickerInfo` + `exchange::TickerInfo` | `data::FuturesTickerInfo` | `data/src/domain/futures.rs` |
| `data::Trade` + `exchange::Trade` | `data::Trade` + `exchange::RawTrade` (wire only) | `data/src/domain/entities.rs`, `exchange/src/types.rs` |
| `data::Candle` + `exchange::Kline` | `data::Candle` + `exchange::RawKline` (wire only) | Same |
| `iced_core::Color` in data crate | `data::Rgba` | `data/src/drawing/types.rs` |
| 3x `FUTURES_PRODUCTS` constants | 1x `data::products::FUTURES_PRODUCTS` | `data/src/products.rs` |
| `study::PriceLevel.price: f64` | `study::PriceLevel.price: data::Price` | `study/src/output.rs` |
| `FuturesTickerInfo.tick_size: f32` | `FuturesTickerInfo.tick_size: Price` | `data/src/domain/futures.rs` |

## Appendix B: Files Deleted in Target Architecture

| Path | Reason |
|---|---|
| `src/chart/study/` (entire directory) | Dead code; `TradeGroup`/`Footprint` moved to candlestick |
| `src/chart/indicator/` (already deleted in git) | Migrated to `kairos-study` crate |
| `src/modals/pane/indicators.rs` (already deleted in git) | Replaced by `indicator_manager.rs` |
| `data/src/secrets/mod.rs` | Moved to GUI crate as `src/secrets.rs` |
| `exchange/src/types.rs::TickerInfo` struct | Replaced by `FuturesTickerInfo` |

## Appendix C: New Files Created in Target Architecture

| Path | Purpose |
|---|---|
| `src/platform.rs` | `data_path()`, `open_data_folder()` — moved from data crate |
| `src/secrets.rs` | `SecretsManager` — moved from data crate |
| `src/app/globals.rs` | OnceLock statics — extracted from `app/mod.rs` |
| `src/app/messages.rs` | Message enums — extracted from `app/mod.rs` |
| `src/app/view.rs` | `Kairos::view()` — extracted from `app/mod.rs` |
| `src/app/sidebar_view.rs` | `view_with_modal()` — extracted from `app/mod.rs` |
| `src/app/ticker_registry.rs` | `build_tickers_info()` — extracted from `app/mod.rs` |
| `src/chart/volume_bar.rs` | `draw_volume_bar()` with `VolumeBarSpec` struct |
| `data/src/products.rs` | Canonical `FUTURES_PRODUCTS` constant |
| `data/src/domain/side.rs` | `TradeSide` and `BookSide` enums |
| `data/src/error.rs` | `DataError` — moved from `lib.rs` |
| `exchange/src/ext/mod.rs` | Extension trait module |
| `exchange/src/ext/databento.rs` | `DatabentoTradeExt` trait |
