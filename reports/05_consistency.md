# API Design & Cross-Crate Consistency Audit

**Codebase**: Kairos — Rust workspace (src/, data/, exchange/, study/)
**Auditor**: API Design & Consistency Agent
**Date**: 2026-02-20

---

## Executive Summary — Top 5 Inconsistency Clusters

1. **Dual Price Type System** — Two completely separate `Price` structs coexist: `data::domain::types::Price` and `exchange::util::Price`. Both are `i64` fixed-point at 10^-8 precision, but they are different types requiring explicit `From` conversions everywhere. Additionally, `FuturesTickerInfo.tick_size` is `f32` (not `Price`), and `study::output` uses raw `f64` for prices in `PriceLevel`, `ProfileLevel`, and `ClusterRow`.

2. **Parallel Trade / Kline Types** — `exchange::types::Trade` (`time: u64`, `price: f32`) and `data::domain::entities::Trade` (`time: Timestamp`, `price: Price`) represent the same concept with completely different field types. Likewise `exchange::types::Kline` vs domain `Candle`. This duality forces costly conversions at every layer boundary.

3. **Inconsistent Error Propagation** — Result types in Message enums (app layer) use `Result<T, String>` everywhere (e.g., `ChartDataLoaded`, `RithmicConnected`). The data/exchange layers correctly use structured error types implementing `AppError`, but the serialization of errors to `String` for cross-thread message passing discards all type information, is_retriable, and severity. Additionally, two parallel error traits exist: `AppError` (data layer) and—per CLAUDE.md—`UserFacingError` (exchange layer), but no `UserFacingError` trait was found in the code; only `AppError` is implemented across exchange and data crates.

4. **Ticker Identifier Inconsistency** — Options repositories (`OptionSnapshotRepository`, `OptionChainRepository`, `OptionContractRepository`) take `underlying_ticker: &str` as plain strings, while futures repositories take `ticker: &FuturesTicker`. This dual approach means option tickers have no type safety: no validation, no venue information, no structured parsing.

5. **TradeRepository Trait Boundary Leakage** — The `TradeRepository` trait, defined in the `data` layer, contains five Databento-specific methods (`check_cache_coverage_databento`, `prefetch_to_cache_databento`, `get_actual_cost_databento`, etc.) with `_databento` suffixes. This leaks exchange-adapter concerns into a domain-level abstraction and violates the repository pattern's purpose.

---

## Detailed Findings

### Category 1: Trait Design

#### Finding 1.1 — Dual `Price` Types
**[SEVERITY: HIGH]**
**Files**: `data/src/domain/types.rs`, `exchange/src/util.rs`

Two `Price` types exist with almost identical internal structure (`units: i64` at `10^-8` precision):

```rust
// data/src/domain/types.rs
pub struct Price { units: i64 }  // private field
impl Price { pub fn units(self) -> i64 { self.units } }

// exchange/src/util.rs
pub struct Price { pub units: i64 }  // public field!
```

Key differences that cause friction:
- `data::Price` has `units` as a private field accessed via `.units()` method; `exchange::Price` has `units` as a public field accessed directly.
- `exchange::Price` has `.to_f32_lossy()` / `.from_f32_lossy()` (explicit about lossiness); `data::Price` has `.to_f32()` / `.from_f32()` (no lossiness warning).
- `exchange::Price` has checked arithmetic (`checked_add` with panics on overflow); `data::Price` uses unchecked arithmetic.
- `exchange::util.rs` also defines `PriceStep` (a newtype for step/tick sizes), while `data::Price` does tick rounding directly via `round_to_tick(tick_size: Price)`.
- `From` impls exist (`exchange::Price` → `data::Price` and vice versa) but every boundary crossing requires explicit conversion.

**Standardization recommendation**: Promote `data::domain::types::Price` to the single canonical type. Have `exchange` re-export `data::Price` and `data::Price::from_units()` / `data::Price::units()`. Deprecate `exchange::util::Price`. The checked arithmetic of `exchange::Price` should be merged into `data::Price`.

#### Finding 1.2 — `TradeRepository` Leaks Databento Concerns
**[SEVERITY: HIGH]**
**File**: `data/src/repository/traits.rs` (lines 155–234)

The `TradeRepository` trait is defined in the `data` layer (no exchange dependency), yet contains five adapter-specific methods:
```rust
async fn check_cache_coverage_databento(&self, ..., schema_discriminant: u16, ...) -> ...;
async fn prefetch_to_cache_databento(&self, ..., schema_discriminant: u16, ...) -> ...;
async fn prefetch_to_cache_databento_with_progress(&self, ...) -> ...;
async fn get_actual_cost_databento(&self, ..., schema_discriminant: u16, ...) -> ...;
async fn list_cached_symbols_databento(&self) -> ...;
```
The parameters even use `schema_discriminant: u16` — a Databento-specific wire-format type. The trait comment acknowledges: `// TODO: Consider extracting to a separate extension trait`.

**Standardization recommendation**: Extract a `DatabentoTradeRepository` extension trait in the `exchange` crate:
```rust
#[async_trait]
pub trait DatabentoExt: TradeRepository {
    async fn check_cache_coverage(&self, ticker: &FuturesTicker, schema: Schema, ...) -> ...;
    // etc.
}
```
Remove all `_databento` methods from the base `TradeRepository` trait.

#### Finding 1.3 — `Chart` Trait Default Methods are Non-Optional
**[SEVERITY: LOW]**
**File**: `src/chart/core/traits.rs`

The `Chart` trait provides eight default-returning methods (e.g., `active_drawing_tool`, `has_pending_drawing`, `hit_test_drawing`, `has_drawing_selection`, `is_drawing_selected`, `has_clone_pending`). All default to `false`/`None`/`DrawingTool::None`. This is acceptable for a trait with optional capabilities, but mixing mandatory methods (`state()`, `mut_state()`, `interval_keys()`, etc.) with optional capability methods in the same trait violates the Interface Segregation Principle. Charts with drawing support override these; charts without simply rely on defaults — which is fine, but not explicitly documented at the trait boundary.

**Standardization recommendation**: Consider a `DrawingCapable` sub-trait:
```rust
pub trait DrawingCapable: Chart {
    fn active_drawing_tool(&self) -> DrawingTool;
    fn has_pending_drawing(&self) -> bool;
    // ...
}
```

#### Finding 1.4 — `Study` Trait: `compute` Returns `()` not `Result`
**[SEVERITY: MEDIUM]**
**File**: `study/src/traits.rs` (line 30)

```rust
fn compute(&mut self, input: &StudyInput);
```

`compute` takes a `&StudyInput` but returns `()` — errors during computation are silently swallowed. There is no way for callers to know if a study failed to compute (e.g., insufficient data, invalid parameters). Compare with `set_parameter` which correctly returns `Result<(), StudyError>`.

**Standardization recommendation**: Change to:
```rust
fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError>;
```
Since `StudyError` is already defined, this is a natural extension.

---

### Category 2: Type Consistency Across Crates

#### Finding 2.1 — f32/f64 Price Leakage in Study Output
**[SEVERITY: HIGH]**
**File**: `study/src/output.rs`

Three output types in the study layer use raw floating-point for prices:

```rust
pub struct PriceLevel { pub price: f64, ... }     // line 113
pub struct ProfileLevel { pub price: f64, ... }   // line 136
pub struct ClusterRow { pub price: f64, ... }     // line 156
```

These violate the established `Price` type invariant. Prices stored as `f64` can accumulate floating-point errors, and there is no protection against invalid values. The `TradeMarker` struct in the same file partially addresses this by using `pub price: i64` (raw units), but the comment says `/// Y position: VWAP in domain Price units (10^-8)` — so the semantics differ from the other structs.

**Standardization recommendation**: Replace `f64` price fields with `data::Price`. For output types that are purely for rendering (and already converted for the canvas), document this clearly and use `f32` consistently (not `f64`) to match GPU/canvas precision.

#### Finding 2.2 — `exchange::types::Trade` Uses Primitive Types
**[SEVERITY: HIGH]**
**File**: `exchange/src/types.rs` (lines 14–20)

```rust
pub struct Trade {
    pub time: u64,   // raw millis, not Timestamp
    pub price: f32,  // not Price
    pub qty: f32,    // not Quantity
    pub side: TradeSide,  // not Side
}
```

This struct duplicates `data::domain::entities::Trade` with weaker types. The exchange layer has its own `TradeSide` enum in addition to `data::Side`. The `Kline` type (lines 29–39) similarly uses `f32` for all price fields rather than `Price`.

This creates a seam where type-safe domain objects are translated to/from primitive types at every exchange adapter boundary, and the translation can fail silently (e.g., truncating price precision).

**Standardization recommendation**: The `exchange::types::Trade` and `exchange::types::Kline` exist because exchange adapters work with raw wire format before conversion. This is acceptable, but the conversion from `exchange::Trade` → `data::Trade` should be a single `From`/`TryFrom` impl, not scattered throughout adapter code.

#### Finding 2.3 — `FuturesTickerInfo.tick_size` is `f32`, not `Price`
**[SEVERITY: MEDIUM]**
**File**: `data/src/domain/futures.rs` (lines 402–408)

```rust
pub struct FuturesTickerInfo {
    pub ticker: FuturesTicker,
    pub tick_size: f32,    // WHY f32 here?
    pub min_qty: f32,
    pub contract_size: f32,
}
```

`FuturesTickerInfo` is a domain type in the data layer. The tick size is converted to `Price` on the fly via `min_ticksize()`:
```rust
pub fn min_ticksize(&self) -> Price {
    Price::from_f32(self.tick_size)
}
```

This round-trip conversion (`f32` → store as `f32` → convert to `Price` each time) is unnecessary. Since `Price` is the canonical representation, `tick_size` should be stored as `Price`.

**Standardization recommendation**: Change `tick_size: f32` to `tick_size: Price` in `FuturesTickerInfo`. Update all construction sites (primarily `build_tickers_info` in `src/app/mod.rs` and the `FUTURES_PRODUCTS` constant).

#### Finding 2.4 — Timestamp Inconsistency
**[SEVERITY: MEDIUM]**
**Files**: `data/src/domain/types.rs`, `exchange/src/types.rs`

The domain layer defines `Timestamp(pub u64)` (a newtype wrapper), but exchange types use raw `u64` for time:
- `exchange::types::Trade.time: u64`
- `exchange::types::Kline.time: u64`
- `exchange::types::Depth.time: u64`
- `study::output::LineSeries.points: Vec<(u64, f32)>` — x-axis is raw `u64`
- `study::output::TradeMarker.time: u64` — raw `u64`

The `Timestamp` newtype is used correctly in domain entities but not in exchange types or study outputs, creating a split.

**Standardization recommendation**: Use `Timestamp` (or at minimum, clearly typed `ms: u64`) in all public-facing types. For study outputs, raw `u64` may be acceptable as a rendering hint (it represents milliseconds OR candle indices), but this dual meaning should be documented via a type alias:
```rust
pub type TimeOrIndex = u64;  // ms for time-based, index for tick-based
```

#### Finding 2.5 — Side Enum Has Four Variants (Semantic Confusion)
**[SEVERITY: LOW]**
**File**: `data/src/domain/types.rs` (lines 320–327)

```rust
pub enum Side { Buy, Sell, Bid, Ask }
```

`Side` mixes trade-side semantics (`Buy`/`Sell`) with orderbook-side semantics (`Bid`/`Ask`). The implementation maps `Buy → 0` and `Bid → 0` (same index), creating an implicit equivalence. The exchange layer defines a separate `TradeSide { Buy, Sell }` with only two variants.

**Standardization recommendation**: Split into two enums:
```rust
pub enum TradeSide { Buy, Sell }
pub enum BookSide { Bid, Ask }
```
This matches the exchange layer's `TradeSide` and eliminates the confusing four-variant `Side` with overlapping semantics.

---

### Category 3: Function Signature Patterns

#### Finding 3.1 — Message Enums Use `Result<T, String>` Instead of Structured Errors
**[SEVERITY: MEDIUM]**
**File**: `src/app/mod.rs`

All async result messages serialize errors to `String`:
```rust
ChartDataLoaded { result: Result<data::ChartData, String> },
RithmicConnected { result: Result<(), String> },
DataDownloadComplete { result: Result<usize, String> },
HistoricalDownloadCostEstimated { result: Result<(usize, usize, usize, String, f64, Vec<chrono::NaiveDate>), String> },
```

This pattern is a well-known Iced/Elm workaround (async tasks must produce `Clone` messages, and error types often aren't `Clone`). But it loses all structured error information. The `(usize, usize, usize, String, f64, Vec<chrono::NaiveDate>)` tuple is particularly opaque — there are no field names.

**Standardization recommendation**: Define dedicated result structs:
```rust
pub struct DownloadCostEstimate {
    pub estimated_days: usize,
    pub cached_days: usize,
    pub missing_days: usize,
    pub cost_str: String,
    pub cost_usd: f64,
    pub missing_dates: Vec<chrono::NaiveDate>,
}
```

For the error side, implement `Clone` on the error types or create a `CloneableError(String, ErrorSeverity, bool)` that preserves severity and is_retriable.

#### Finding 3.2 — Inconsistent Parameter Naming: `ticker` vs `ticker_info`
**[SEVERITY: LOW]**
**Multiple files**

Functions taking ticker information use inconsistent parameter names:
- `fn new_for_kind(kind, ticker_info: FuturesTickerInfo, settings)` — `ticker_info`
- `fn get_trades(ticker: &FuturesTicker, date_range)` — `ticker`
- `fn get_depth(ticker: &FuturesTicker, date_range)` — `ticker`
- `fn build_tickers_info(available_symbols: HashSet<String>)` — takes `String`, returns `FxHashMap<FuturesTicker, FuturesTickerInfo>`

The naming is contextually appropriate but not systematic. A convention document would help.

#### Finding 3.3 — Constructor Naming Inconsistency
**[SEVERITY: LOW]**
**Multiple files**

Constructors use inconsistent naming:
- `Trade::new(...)` — conventional
- `Trade::from_raw(...)` — special raw constructor
- `Depth::new(time: u64)` — takes only time, rest defaulted
- `Candle::new(time, open, high, low, close, buy_volume, sell_volume)` — full constructor
- `RepositoryStats::new()` — zero-argument, could be `Default`

`RepositoryStats::new()` is identical to `RepositoryStats::default()` (which also exists). Having both is redundant.

---

### Category 4: Enum Design

#### Finding 4.1 — `ContentKind` vs `Content` Duplication
**[SEVERITY: MEDIUM]**
**Files**: `data/src/state/pane.rs`, `src/screen/dashboard/pane/content.rs`

`ContentKind` (in `data/`) and `Content` (in `src/`) are parallel enums with the same six variants: `Starter`, `HeatmapChart`, `CandlestickChart`, `TimeAndSales`, `Ladder`, `ComparisonChart`. Methods like `Content::kind()` and `Content::placeholder(kind)` bridge between them. Synchronization bugs are possible — if a new variant is added to one, the other must be manually updated.

`ContentKind::to_chart_type()` has a semantic issue:
```rust
ContentKind::TimeAndSales => ChartType::Candlestick,  // TimeAndSales maps to Candlestick?
ContentKind::Ladder => ChartType::Candlestick,         // same
ContentKind::Starter => ChartType::Candlestick,        // same
```
Non-chart content kinds arbitrarily map to `Candlestick`. This method appears to be a workaround rather than clean design.

**Standardization recommendation**: Move `Content` to the `data` crate as a typed enum variant or replace the parallel enum with a trait-based approach. At minimum, derive a macro or add a test ensuring both enums remain in sync.

#### Finding 4.2 — `StudyOutput` Variants are Incompletely Serializable
**[SEVERITY: LOW]**
**File**: `study/src/output.rs`

Most `StudyOutput` sub-types derive `Serialize, Deserialize`, but:
- `StudyOutput` itself does NOT derive `Serialize/Deserialize`
- `TradeMarker` does NOT derive `Serialize/Deserialize` (intentional—used only at runtime)
- `TradeMarkerDebug` does NOT derive `Serialize/Deserialize`

This inconsistency means the output cannot be cached or transmitted as-is. If persistence of study outputs is ever needed, the derives will need to be added carefully.

#### Finding 4.3 — `Timeframe` Variant Naming Convention Breaks at 3 Levels
**[SEVERITY: LOW]**
**File**: `data/src/domain/futures.rs` (lines 454–481)

```rust
pub enum Timeframe {
    M1s,  // "M" prefix but it's seconds (s)
    M5s,
    M1,   // "M" prefix, actual minutes
    M3,
    H1,   // "H" prefix, hours
    D1,   // "D" prefix, days
}
```

`M1s`, `M5s` use `M` prefix inconsistently — `M` typically means minutes, but these are seconds (indicated by the `s` suffix). The pattern `M1s` reads as "1 minute second". A cleaner naming would be `S1`, `S5`, `S10`, `S30` for seconds.

---

### Category 5: Message Hierarchy

#### Finding 5.1 — Message Hierarchy is Well-Structured
**[SEVERITY: INFO]**

The Elm message routing is generally clean:
```
Message (src/app/mod.rs)
  └─ Message::Dashboard { layout_id, event: dashboard::Message }
       └─ dashboard::Message::Pane(window::Id, pane::Message)
            └─ pane::Message::PaneEvent(pane_grid::Pane, Event)
                 └─ Event::ChartInteraction(chart::Message)
```

This four-level hierarchy is well-encapsulated. Each level handles its own domain.

#### Finding 5.2 — `pane::Message` Mixes Layout and Content Messages
**[SEVERITY: MEDIUM]**
**File**: `src/screen/dashboard/pane/types.rs`

`pane::Message` contains both pane grid layout messages (structural) and content/event messages (behavioral):
```rust
pub enum Message {
    // Layout / grid (structural):
    PaneClicked, PaneResized, PaneDragged, ClosePane, SplitPane, MaximizePane, Restore, ReplacePane, Popout, Merge,
    // Content-level (behavioral):
    SwitchLinkGroup, VisualConfigChanged, PaneEvent,
}
```

`PaneEvent(pane_grid::Pane, Event)` wraps the inner `Event` enum (which contains chart interactions, modal interactions, etc.), creating a two-level message type in one enum. This is slightly awkward — callers must construct `Message::PaneEvent(pane, Event::ChartInteraction(chart::Message::Translated(...)))` which is deeply nested.

#### Finding 5.3 — `sidebar::Message` Skips Dashboard Level for Some Actions
**[SEVERITY: LOW]**
**File**: `src/app/mod.rs`, `src/screen/dashboard/sidebar.rs`

`Message::Sidebar(dashboard::sidebar::Message)` routes directly to the sidebar without going through `dashboard::Message`. This is intentional (sidebar is at the app level, not inside the dashboard), but creates an asymmetry: some pane-level operations route through `Message::Dashboard { event: dashboard::Message }`, while sidebar interactions skip the dashboard wrapper entirely.

---

### Category 6: Serialization Consistency

#### Finding 6.1 — `DepthSnapshot` Uses Domain `Price` for Keys But `exchange::Depth` Uses `i64`
**[SEVERITY: MEDIUM]**
**Files**: `data/src/domain/entities.rs`, `exchange/src/types.rs`

```rust
// data layer:
pub struct DepthSnapshot {
    pub bids: BTreeMap<Price, Quantity>,  // Price keys
    pub asks: BTreeMap<Price, Quantity>,
}

// exchange layer:
pub struct Depth {
    pub bids: BTreeMap<i64, f32>,  // raw units as i64 keys, f32 qty
    pub asks: BTreeMap<i64, f32>,
}
```

The exchange's `Depth` type uses raw `i64` (price units) as map keys and `f32` for quantity. The domain's `DepthSnapshot` uses `Price` and `Quantity` newtypes. These must be converted at every boundary. The `exchange::Depth` explicitly comments `// price_units -> quantity`, which indicates awareness of the mismatch.

**Standardization recommendation**: Either make `exchange::Depth` use `BTreeMap<data::Price, f32>` (since `data::Price` is `Ord` and `Hash`), or keep the current split but ensure the conversion is done once at a clearly defined boundary.

#### Finding 6.2 — `FuturesTicker` Has Custom `Serialize`/`Deserialize` With Hardcoded String
**[SEVERITY: MEDIUM]**
**File**: `data/src/domain/futures.rs` (lines 347–397)

`FuturesTicker`'s serialization uses a hardcoded `"CMEGlobex"` string:
```rust
impl Serialize for FuturesTicker {
    fn serialize<S>(&self, serializer: S) -> ... {
        let venue_str = "CMEGlobex";  // HARDCODED
```

If a second venue is ever added, the serialize impl would need updating separately from `FuturesVenue::dataset()` and `fmt::Display`. The deserialize arm correctly parses the venue string, but the serialize arm bypasses `FuturesVenue`'s own display format.

**Standardization recommendation**: Use `self.venue.to_string()` instead of the hardcoded string, ensuring consistency with `FuturesVenue`'s own `Display` impl.

#### Finding 6.3 — `serde(rename)` on `Timeframe` But Manual `Display`
**[SEVERITY: LOW]**
**File**: `data/src/domain/futures.rs`

`Timeframe` uses `#[serde(rename = "1s")]` for serialization but has a manual `Display` that also produces the same strings (`"1s"`, `"1m"`, etc.). These two sources of truth (serde rename and Display) can diverge. If a variant is renamed in serde but not in Display (or vice versa), the display would differ from the serialized form.

**Standardization recommendation**: Implement `Display` by deserializing from the serde name (or use `strum` crate for deriving both from a single attribute).

#### Finding 6.4 — `AppState` Version Uses `u32` Field, Not the `StateVersion` Newtype
**[SEVERITY: LOW]**
**File**: `data/src/state/persistence.rs`

`StateVersion` is a newtype `StateVersion(pub u32)`, but `AppState.version` is stored as `u32`:
```rust
if state.version < StateVersion::CURRENT.0 {  // comparing u32 to u32
    state.version = StateVersion::CURRENT.0;   // setting u32
}
```

The `StateVersion` newtype is constructed around `u32` constants but the actual stored field appears to be `u32`. This means the newtype provides no value — all comparisons unwrap to `.0`.

**Standardization recommendation**: Either use `StateVersion` throughout (`state.version: StateVersion`), or drop the newtype and use `u32` consistently with a `STATE_VERSION: u32 = 2` constant.

---

### Category 7: Import & Re-export Patterns

#### Finding 7.1 — `data::lib.rs` Re-exports are Complete But Include Internal Types
**[SEVERITY: LOW]**
**File**: `data/src/lib.rs`

`data::lib.rs` re-exports over 50 types at the crate root. Most are appropriate public API items, but some implementation-detail types like `Axis`, `Layouts`, `ChartState` are re-exported at the top level, which may encourage coupling to internal types. The `pub use config::sidebar;` re-exports the entire `sidebar` module, which may expose more than intended.

#### Finding 7.2 — Mixed HashMap Usage: `std::HashMap`, `FxHashMap`, `rustc_hash::FxHashMap`
**[SEVERITY: LOW]**
**Files**: `src/app/mod.rs`, multiple

The application layer uses both `std::collections::HashMap` and `rustc_hash::FxHashMap` (imported as `use rustc_hash::FxHashMap`). `Kairos` struct itself has:
```rust
pub(crate) tickers_info: FxHashMap<FuturesTicker, FuturesTickerInfo>,
```
While `DOWNLOAD_PROGRESS` uses:
```rust
HashMap<uuid::Uuid, (usize, usize)>
```

This inconsistency means performance-sensitive collections (ticker lookups) use `FxHashMap` while others use the standard `HashMap`. The choice is not documented.

**Standardization recommendation**: Establish a crate-wide alias: `type Map<K, V> = rustc_hash::FxHashMap<K, V>` or document explicitly when to use each.

---

### Category 8: Options Repository Type Inconsistency

#### Finding 8.1 — Options Repositories Use `&str` for Tickers, Futures Use `&FuturesTicker`
**[SEVERITY: MEDIUM]**
**File**: `data/src/repository/traits.rs`

```rust
// Futures repos — typed
async fn get_trades(&self, ticker: &FuturesTicker, date_range: &DateRange) -> ...;
async fn get_depth(&self, ticker: &FuturesTicker, date_range: &DateRange) -> ...;

// Options repos — untyped strings
async fn get_snapshots(&self, underlying_ticker: &str, date_range: &DateRange) -> ...;
async fn get_chain(&self, underlying_ticker: &str, date: NaiveDate) -> ...;
async fn get_contracts(&self, underlying_ticker: &str) -> ...;
async fn search_contracts(&self, underlying_ticker: Option<&str>, ...) -> ...;
```

Options ticker identifiers are just strings with no validation or type safety. The domain layer defines `OptionContract` with a `ticker` field (presumably a string), but the string-based approach could easily admit malformed ticker symbols.

**Standardization recommendation**: If options tickers follow a different structure from futures tickers (they do — e.g., `AAPL` for equity vs `ES.c.0` for futures), create an `EquityTicker` newtype rather than using raw `&str`. At minimum, validate format at repository entry points.

---

## Proposed Style Guide Rules

To prevent future inconsistencies, the following rules should be codified:

### Rule 1: Single Price Type
All prices MUST use `data::Price` (i64, 10^-8 precision). Raw `f32`/`f64` for prices is only permitted in:
- Canvas/GPU rendering coordinates (explain this with a comment)
- `From`/`TryFrom` conversion boundaries between layers (single explicit conversion point)
- Study output types that are purely for rendering (document with a `// RENDER-ONLY` comment)

### Rule 2: Typed Identifiers
All identifier types that cross API boundaries MUST be newtypes:
- Futures: `FuturesTicker` (existing)
- Options equity: `EquityTicker` (to be created)
- Pane IDs: `uuid::Uuid` (existing, acceptable)
- Feed IDs: `FeedId` (existing)
- Strings as identifiers are NOT acceptable in public API signatures

### Rule 3: Error Propagation
- Data/exchange layer errors MUST implement `AppError` with user_message(), is_retriable(), severity()
- Cross-thread message errors MAY use `String` but MUST preserve severity in the error string (e.g., `"[ERROR] Failed to fetch data: ..."`)
- Structured error types that are `Send + 'static` SHOULD implement `Clone` to avoid the `String` conversion

### Rule 4: Trait Boundary Purity
- The `data` layer traits MUST NOT reference exchange-specific types (no `schema_discriminant: u16`, no `_databento` methods)
- Exchange-specific capabilities extend base traits via separate extension traits in the `exchange` crate

### Rule 5: Naming Conventions
- Constructors: Use `new()` for the primary constructor; `from_*()` for alternate construction from other types; `with_*()` for builder-style construction
- Parameter names: `ticker: &FuturesTicker` for typed ticker parameters; `ticker_info: FuturesTickerInfo` when the full info struct is needed
- Timeframe variants: Use consistent prefix: `S1`, `S5`, ... for seconds; `M1`, `M5`, ... for minutes; `H1`, `H4` for hours; `D1` for day

### Rule 6: Serialization
- Avoid hardcoded strings in custom Serialize impls; use the type's Display or a mapping function
- Serde `rename` attributes and `Display` implementations MUST produce the same strings for the same variants
- Version numbers in persisted state MUST be stored as the `StateVersion` newtype, not as raw `u32`

### Rule 7: HashMap Consistency
- Performance-sensitive lookup tables MUST use `rustc_hash::FxHashMap`
- Order-dependent maps MUST use `BTreeMap`
- Default to `FxHashMap` over `std::HashMap` for new code
- Add `type HashMap<K, V> = rustc_hash::FxHashMap<K, V>` as a crate-level alias in `src/lib.rs`

---

## Summary Table

| Finding | Severity | Category | Effort to Fix |
|---------|----------|----------|---------------|
| 1.1 Dual Price types | HIGH | Trait Design | High |
| 1.2 TradeRepository leaks Databento | HIGH | Trait Design | Medium |
| 2.1 f64 price in study output | HIGH | Type Consistency | Medium |
| 2.2 exchange::Trade uses f32 | HIGH | Type Consistency | High |
| 2.3 FuturesTickerInfo.tick_size is f32 | MEDIUM | Type Consistency | Low |
| 2.4 Timestamp inconsistency | MEDIUM | Type Consistency | High |
| 3.1 Result<T, String> in messages | MEDIUM | Signatures | Medium |
| 4.1 ContentKind/Content duplication | MEDIUM | Enum Design | Medium |
| 4.2 StudyOutput partial serialization | LOW | Enum Design | Low |
| 5.2 pane::Message mixes layout/content | MEDIUM | Message Hierarchy | Medium |
| 6.1 Depth map key types | MEDIUM | Serialization | Medium |
| 6.2 FuturesTicker hardcoded venue string | MEDIUM | Serialization | Low |
| 8.1 Options repos use &str tickers | MEDIUM | Type Safety | Medium |
| 1.3 Chart trait non-optional defaults | LOW | Trait Design | Low |
| 1.4 Study compute returns () | MEDIUM | Trait Design | Low |
| 2.5 Side has 4 variants | LOW | Type Consistency | Medium |
| 3.3 Constructor naming | LOW | Signatures | Low |
| 4.3 Timeframe naming | LOW | Enum Design | Low |
| 6.3 Timeframe serde/Display | LOW | Serialization | Low |
| 6.4 StateVersion not used uniformly | LOW | Serialization | Low |
| 7.1 Re-export scope | LOW | Imports | Low |
| 7.2 Mixed HashMap types | LOW | Imports | Low |
