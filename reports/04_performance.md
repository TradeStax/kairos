# Performance & Optimization Audit — Kairos

**Scope**: `src/chart/`, `src/screen/`, `data/src/services/`, `exchange/src/`
**Date**: 2026-02-20

---

## Executive Summary — Top 5 Bottlenecks

| Rank | Area | Issue | Estimated Impact |
|------|------|-------|-----------------|
| 1 | Heatmap `draw()` | `visible_data_count` computed twice per frame with two separate BTreeMap range scans | High (10–50ms per frame during busy markets) |
| 2 | Heatmap depth processing | O(N) linear scan inside `add_trade()` for grouping (`iter_mut().find()`) | High (10–100ms for large trade sets) |
| 3 | Replay engine / replay actions | `block_on()` called directly inside `spawn_blocking` closures — nested runtime blockage | Critical (can freeze UI thread) |
| 4 | Aggregation: O(N²) sort validation | `aggregate_trades_to_candles` calls `windows(2)` for a full sort check before building a BTreeMap | Medium (1–10ms at 1M trades) |
| 5 | Volume profile recomputed every frame | `draw_volume_profile()` rebuilds the entire profile `Vec` inside the cached `draw` closure on every main-cache invalidation | High (10–50ms per frame with large visible range) |

---

## Detailed Findings

---

### 1. [HIGH] Duplicate `visible_data_count` Computation in Heatmap `draw()`

**File**: `src/chart/heatmap/render.rs:86–98` and `render.rs:428–440`

The `draw()` function computes `visible_data_count` (a BTreeMap `.range(earliest..=latest).map(|dp| dp.grouped_trades.len()).sum()`) in two places within the same frame:

1. At line 86 to compute the LOD level for depth rendering.
2. At line 428 inside `draw_trade_markers()` to determine rendering mode.

Both of these perform an identical range scan over `heatmap_data.trades_by_time`, iterating all buckets in range and summing trade counts. With a busy ES feed producing thousands of depth updates per second and hundreds of aggregated trade buckets, this doubles work done inside the canvas `Cache::draw()` closure.

**Optimization**: Compute `visible_data_count` once before calling `draw_depth_heatmap` and `draw_trade_markers`, and pass it as a parameter. This eliminates one full BTreeMap range scan + summation per frame.

```rust
// Compute ONCE
let visible_data_count: usize = self
    .heatmap_data
    .trades_by_time
    .range(earliest..=latest)
    .map(|(_, dp)| dp.grouped_trades.len())
    .sum();
let lod = LodCalculator::new(chart.scaling, chart.cell_width, visible_data_count, region.width);
let lod_level = lod.calculate_lod();

// Pass to draw_trade_markers instead of recomputing
draw_trade_markers(..., visible_data_count, ...);
```

---

### 2. [HIGH] O(N) Linear Scan Inside `HeatmapData::add_trade()`

**File**: `src/chart/heatmap/data.rs:183–195`

```rust
if let Some(existing) = entry
    .grouped_trades
    .iter_mut()
    .find(|t| t.price == price_rounded && t.is_sell == is_sell)
{
    existing.qty += qty;
} else {
    entry.grouped_trades.push(GroupedTrade { ... });
}
```

`grouped_trades` is a `Vec<GroupedTrade>`. For each incoming trade, this performs a linear scan over all existing grouped trades in the same time bucket to find a match. In the worst case (many distinct price levels in one time bucket), this is O(M) where M is the number of distinct (price, side) pairs per bucket.

For an active ES session this can be called millions of times during heatmap construction (`from_chart_data`) and during replay (`append_trade`). With highly active price levels, M can be 50–200 per bucket, making initialization O(N×M) rather than O(N).

**Optimization**: Replace the `Vec<GroupedTrade>` with a small `BTreeMap<(i64, bool), f32>` keyed on `(price_units, is_sell)` to get O(log M) lookup. Alternatively, since M is typically small and cache-friendly, use a sorted Vec with binary search. Only convert back to Vec for rendering if needed.

---

### 3. [CRITICAL] `block_on()` Inside `spawn_blocking()` — Replay Engine

**File**: `src/app/update/replay.rs:186–197`, `replay.rs:223–260`, `replay.rs:286–300`

The pattern:

```rust
tokio::task::spawn_blocking(move || {
    let mut guard = engine.lock().unwrap_or_else(|e| e.into_inner());
    tokio::runtime::Handle::current().block_on(engine.play())
})
```

...and inside `replay_load_data()`:

```rust
tokio::task::spawn_blocking(move || {
    let handle = tokio::runtime::Handle::current();
    ...
    handle.block_on(async {
        guard.load_data(ticker_info, date_range).await?;
        guard.seek(ts).await?;
        guard.play().await?;
        ...
    })
})
```

`spawn_blocking` is meant for CPU-bound synchronous work, but here the inner closure immediately calls `block_on()` on async futures (I/O-bound operations). This occupies a blocking thread while also potentially starving the async runtime because the blocking thread count is limited and `block_on` will block the OS thread.

Additionally, `replay_engine_action()` at lines 17–22 calls `block_on(engine.play())`, `block_on(engine.pause())`, `block_on(engine.stop())`, etc., directly — each inside `spawn_blocking`, producing nested runtime interaction that can deadlock in single-threaded contexts.

**Optimization**: Replay engine operations (`play`, `pause`, `stop`, `seek`, `set_speed`) should be driven via `Task::perform(async move { engine.lock().await.play().await })` using Tokio's standard async execution path. The `spawn_blocking` wrapper should be reserved only for the initial data loading step that performs CPU-bound aggregation, not for I/O or async coordination.

---

### 4. [MEDIUM] O(N) Sort Validation in Trade Aggregation

**File**: `data/src/domain/aggregation.rs:89–94` and `aggregation.rs:204–209`

Both `aggregate_trades_to_candles` and `aggregate_trades_to_ticks` perform:

```rust
for window in trades.windows(2) {
    if window[0].time > window[1].time {
        return Err(AggregationError::UnsortedTrades);
    }
}
```

This is a full O(N) pass over potentially millions of trades just to validate ordering, before the actual aggregation work begins. For a typical day of ES data (~2–5M trades) this adds a measurable overhead on every chart load and every basis switch.

**Optimization**: Databento data is guaranteed to arrive pre-sorted from the API (and verified at ingest). Remove the sort validation from the hot path. Either move it to an `#[cfg(debug_assertions)]` gate, or trust the repository layer to deliver sorted data (as is already the assumption in `rebuild_chart_data` which calls `aggregate_to_basis` without re-checking).

---

### 5. [HIGH] Volume Profile Rebuilt Every Frame

**File**: `src/chart/heatmap/render.rs:644–688`

Inside the `main` cache `draw()` closure, `draw_volume_profile()` is called when a `VolumeProfile` study is active. This function:
1. Computes `num_ticks` (price levels in visible range)
2. Allocates `let mut profile = vec![(0.0f32, 0.0f32); num_ticks]`
3. Iterates all `heatmap_data.trades_by_time` entries in the time range
4. For each trade, rounds to the nearest tick and accumulates into the profile Vec

This is O(B × T) where B is visible time buckets and T is trades per bucket. The whole computation runs inside the `Cache::draw` closure, meaning it is re-executed on every main cache invalidation — including cursor moves if the axis label caches trigger a main cache clear.

**Optimization**: The volume profile is not view-specific in the same sense as crosshair position. Precompute the profile during `invalidate()` (or on data update), store it as `Option<Vec<(f32, f32)>>` in the chart struct, and pass it directly to `draw_volume_profile` for pure rendering. This transforms profile computation from O(frame × data) to O(data_update).

---

### 6. [MEDIUM] Candle Count via `.iter().filter().count()` in LOD Calculation

**File**: `src/chart/candlestick/render.rs:76–87`

```rust
let visible_candle_count = match &self.basis {
    ChartBasis::Time(_) => self
        .chart_data
        .candles
        .iter()
        .filter(|c| c.time.0 >= earliest && c.time.0 <= latest)
        .count(),
    ...
};
```

Every frame, the LOD calculation counts visible candles by iterating the full candle slice. For 10,000 candles and a typical trading day this is 10K comparisons per frame. Since candles are sorted by time, binary search should be used instead.

**Optimization**: Use binary search to find the first candle at or after `earliest` and the last candle at or before `latest`, then count via index subtraction — O(log N) instead of O(N).

```rust
let first = candles.partition_point(|c| c.time.0 < earliest);
let last = candles.partition_point(|c| c.time.0 <= latest);
let visible_candle_count = last - first;
```

---

### 7. [MEDIUM] `draw_crosshair_tooltip` Uses Linear Scan on Every Mouse Move

**File**: `src/chart/candlestick/render.rs:507`

```rust
let candle_opt = match basis {
    ChartBasis::Time(_) => candles.iter().find(|c| c.time.0 == at_interval)...
```

`candles.iter().find(...)` is O(N) and is called inside the `crosshair` cache `draw()` closure which fires on every cursor movement. With 10,000 candles this is 10K comparisons per mouse move event.

**Optimization**: Since `candles` is sorted by time, use binary search (`candles.binary_search_by_key(&at_interval, |c| c.time.0)`). This is O(log N).

---

### 8. [MEDIUM] DepthRun Accumulation Unbounded for Long Sessions

**File**: `src/chart/heatmap/data.rs:131–157`

Each call to `add_depth_snapshot()` appends a `DepthRun` to `depth_by_price[price_units]`. For a single trading day with an active ES orderbook at 10 depth updates/second over 23,000 seconds, and 20 price levels per snapshot: that's 23,000 × 10 × 20 = 4.6M `DepthRun` allocations, all kept in memory.

The `iter_depth_filtered` function then iterates all runs per price level checking time intersections. As sessions grow, this inner iteration cost grows linearly.

**Optimization**: Merge adjacent depth runs at the same price and side when the quantity matches. Or cap the history window and evict runs older than N intervals. Also consider coalescing runs during `add_depth_snapshot` when the previous run at the same price has the same `qty` and `is_bid`.

---

### 9. [MEDIUM] Comparison Chart: O(N) Linear Scan for `idx_right`

**File**: `src/chart/comparison/render.rs:29`, `src/chart/comparison/types.rs:233`

```rust
let idx_right = pts.iter().position(|(x, _)| *x >= ctx.min_x);
```

`pts` is a `Vec<(u64, f32)>` of time-series points. Each render call (and each `interpolate_y_at` call) linearly scans from the beginning. For long comparison series with millions of points, this is expensive. Since points are ordered by time (x), binary search via `partition_point` would give O(log N).

**Optimization**: Replace `iter().position()` with `partition_point(|(x, _)| x < ctx.min_x)`.

---

### 10. [LOW] String Allocations in Hot Render Path

**File**: `src/chart/candlestick/render.rs:546–550`

```rust
let open_str  = format!("{:.prec$}", candle.open.to_f32(),  prec = precision);
let high_str  = format!("{:.prec$}", candle.high.to_f32(),  prec = precision);
let low_str   = format!("{:.prec$}", candle.low.to_f32(),   prec = precision);
let close_str = format!("{:.prec$}", candle.close.to_f32(), prec = precision);
let pct_str   = format!("{change_pct:+.2}%");
```

These `format!` calls allocate heap strings every frame for every cursor position change. While modest individually, they add up in tight loop scenarios.

**Optimization**: Use `write!` into a stack-allocated `ArrayString` or `SmallVec<u8>` buffer. For precision 2 prices, maximum length is bounded (~12 chars), making stack allocation viable. Alternatively use `itoa` or similar for numeric-only parts.

---

### 11. [LOW] `label.clone()` in Marker Render Loop

**File**: `src/chart/study_renderer/markers.rs:90`

```rust
frame.fill_text(Text {
    content: label.clone(),
    ...
});
```

Each marker label is cloned from `Option<String>` for every `fill_text` call. If `fill_text` accepts a `Cow<str>` or `&str`, the clone is unnecessary.

**Optimization**: Check if the Iced `fill_text` API accepts `&str` or `Cow<str>`. If so, pass a reference or `Cow::Borrowed(label)` instead of cloning.

---

### 12. [LOW] `get_all_loading_statuses()` Clones Entire HashMap

**File**: `data/src/services/market_data.rs:401–407`

```rust
pub fn get_all_loading_statuses(&self) -> HashMap<String, LoadingStatus> {
    let status_map = self.loading_status.lock()...;
    status_map.clone()
}
```

This is polled every 500ms via subscription and clones the full map on every poll. For a small number of active operations this is negligible, but it's a pattern to watch.

**Optimization**: Return only the active/in-progress statuses, filtering before clone. Or expose a `for_each_status` callback that runs while holding the lock to avoid the clone entirely.

---

### 13. [LOW] Rithmic Depth/Trade Cloning in Message Fan-out

**File**: `src/app/update/feeds.rs:496–497`

```rust
exchange::Event::DepthReceived(stream_kind, ts, depth.clone(), trades.clone())
```

`DepthSnapshot` contains `Vec<(Price, Volume)>` for bids and asks (up to 10 levels for MBP-10). `trades` is `Vec<Trade>`. These are cloned to construct a new `Event` variant for the dashboard message. For high-frequency depth updates, this is a steady allocation cost.

**Optimization**: Consider wrapping `DepthSnapshot` in `Arc<DepthSnapshot>` so fan-out to multiple panes shares the allocation. This is already partially addressed by the two-level event staging, but the clone at the `Event` construction site creates unnecessary copies.

---

### 14. [LOW] 50ms Polling Loop for Rithmic Events Wakes CPU Unnecessarily When Idle

**File**: `src/app/subscriptions.rs:9–27`

The `rithmic_event_monitor` and `replay_event_monitor` subscriptions unconditionally sleep for 50ms and then lock the global buffer. When no Rithmic connection is active and no replay is running, these subscriptions still spin every 50ms, locking and immediately unlocking two global Mutexes. With six subscriptions running simultaneously (tick at 100ms, status_poll at 500ms, download_poll, rithmic_poll, replay_poll, hotkeys), the application generates 50–100 wakeups/second even when idle.

**Optimization**: Add a fast short-circuit check by using an `AtomicBool` flag that is set only when data is flowing, to skip the lock acquisition when no events are expected. Or use `tokio::sync::Notify` to wake the monitor only when events are pushed.

---

## Caching Effectiveness

**Cache Architecture Summary**:

The `Caches` struct (`src/chart/core/caches.rs`) provides five independent invalidation layers:
- `main` — chart content (candles, depth, volume bars)
- `drawings` — user annotations (rarely changes)
- `x_labels` / `y_labels` — axis labels (contain crosshair position!)
- `crosshair` — cursor overlay

**Key Finding**: The axis label caches (`x_labels`, `y_labels`) include the crosshair's current price/time label in their draw closures. This means every cursor move calls `clear_crosshair()` which clears all three of `crosshair`, `y_labels`, and `x_labels`. The main chart cache is correctly left intact during cursor moves.

This is a reasonable design, but it means the Y-axis label cache (which can contain many rendered price ticks) is regenerated on every mouse move. If the axis label rendering is heavy (many tick lines, formatted numbers), this could be a source of latency.

**Per-Day Disk Cache**: The Databento `.dbn.zst` cache is effective — cache hits avoid all API calls. No issues identified with the caching strategy itself.

**QtyScale Cache**: The `qty_scale_cache: Cell<Option<(u64, u64, i64, i64, QtyScale)>>` in `HeatmapChart` caches quantity scales keyed on `(earliest, latest, highest_units, lowest_units)`. This correctly invalidates when the visible region changes, avoiding three full dataset scans per frame. This is a well-designed pattern.

---

## Startup Performance

**Eager Initialization**:
- `MarketDataService::new()` and `CacheManagerService::new()` are lightweight (no I/O on creation).
- Service initialization in `src/app/services.rs` blocks startup on `HistoricalDataManager::new()` which performs filesystem setup for the cache directory.
- The Rithmic initialization (`rithmic_init_and_stage`) is correctly deferred to user action.
- Ticker info (`FxHashMap<FuturesTicker, FuturesTickerInfo>`) is pre-populated at startup via `build_tickers_info()` — this is a static enumeration and fast.

**Startup Assessment**: No significant startup bottlenecks identified beyond the unavoidable API/cache directory setup.

---

## Async & Concurrency Summary

| Location | Pattern | Risk |
|---|---|---|
| `replay.rs:17,22,37,50,58,63` | `spawn_blocking` + `block_on` for async engine ops | Critical — can deadlock/starve runtime |
| `replay.rs:241` | `handle.block_on(async { guard.load_data(...).await })` inside `spawn_blocking` | High — blocks OS thread during I/O |
| `feeds.rs:553` | `handle.block_on(rithmic_init_and_stage)` inside `spawn_blocking` | Acceptable (one-time init) |
| `data_feeds_manager.lock()` | Locked on UI thread update calls (sync Mutex) | Low — not held across await points |
| `loading_status.lock()` in progress callback | Locked inside async task spawned by `Task::perform` | Low — short critical section |

---

## Summary Table

| # | Impact | File:Line | Issue | Fix |
|---|--------|-----------|-------|-----|
| 1 | High | `heatmap/render.rs:86,428` | `visible_data_count` computed twice per frame | Compute once, pass to both functions |
| 2 | High | `heatmap/data.rs:183` | O(N) Vec scan in `add_trade()` | Use BTreeMap or binary search for grouping |
| 3 | Critical | `update/replay.rs:186,223` | `block_on()` inside `spawn_blocking` for async ops | Use `Task::perform` with proper async |
| 4 | Medium | `domain/aggregation.rs:89,204` | Full O(N) sort validation per load | Move to debug builds only |
| 5 | High | `heatmap/render.rs:645` | Volume profile rebuilt inside draw closure | Precompute on data update, cache result |
| 6 | Medium | `candlestick/render.rs:76` | O(N) visible candle count per frame | Binary search with `partition_point` |
| 7 | Medium | `candlestick/render.rs:507` | O(N) candle find on every cursor move | Binary search |
| 8 | Medium | `heatmap/data.rs:131` | Unbounded `DepthRun` accumulation | Coalesce identical consecutive runs |
| 9 | Medium | `comparison/render.rs:29` | O(N) `position()` scan per render | `partition_point` binary search |
| 10 | Low | `candlestick/render.rs:546` | `format!` heap allocs per cursor move | Stack buffer / `write!` |
| 11 | Low | `study_renderer/markers.rs:90` | `label.clone()` per marker | Pass `&str` if API supports it |
| 12 | Low | `services/market_data.rs:401` | Full HashMap clone on 500ms poll | Filter before clone |
| 13 | Low | `update/feeds.rs:496` | Depth/trade clone per message fan-out | `Arc<DepthSnapshot>` |
| 14 | Low | `subscriptions.rs:9` | 50ms poll even when no feed active | `AtomicBool` guard or `Notify` |
