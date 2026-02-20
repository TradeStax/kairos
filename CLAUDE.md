# CLAUDE.md

Kairos is a native desktop charting platform for futures markets built with Rust and Iced (v0.14). Provides real-time and historical market data visualization: candlestick/footprint charts, heatmaps, order flow analysis, comparison charts, and options data. Supports multi-window layouts with popout panes.

## Build & Test

```bash
cargo build                          # Dev build
cargo build --release                # Release build
cargo run --release                  # Run app
cargo test                           # All tests
cargo test --package kairos-data
cargo test --package kairos-exchange
cargo test --package kairos-study
cargo clippy                         # Lint
cargo fmt --check                    # Format check
```

## Environment Variables

```bash
DATABENTO_API_KEY=your_key           # Required for historical futures data
MASSIVE_API_KEY=your_polygon_key     # Optional for options data
RUST_LOG=kairos_data=debug      # Logging level
```

Rithmic credentials are managed via `keyring` (OS credential store), configured through the UI.

## Architecture

Four workspace crates (Rust edition 2024):

```
app/src/                # Application layer — kairos (Iced GUI)
├── app/               # Kairos struct, message enums, update routing, services
│   ├── globals.rs     # OnceLock statics: DOWNLOAD_PROGRESS, RITHMIC_EVENTS, REPLAY_EVENTS
│   ├── messages.rs    # Message, ChartMessage, DownloadMessage, OptionsMessage
│   ├── view.rs        # Kairos::view() — top-level view dispatch
│   ├── sidebar_view.rs # view_with_modal() — sidebar modal rendering
│   ├── ticker_registry.rs # build_tickers_info(), futures product list
│   └── update/        # Message handlers: chart, download, feeds, navigation, options, preferences, replay
├── screen/dashboard/  # Main dashboard: pane grid, sidebar, panels (ladder, time&sales)
│   ├── pane/          # Pane state, content types, lifecycle, view rendering
│   ├── panel/         # Side panels: Ladder, TimeAndSales
│   ├── layout/        # Pane grid layout management
│   └── loading/       # Loading states & feed management
├── chart/             # Charting engine
│   ├── core/          # Chart trait, ViewState, Caches, Interaction (pan/zoom/drawing)
│   ├── candlestick/   # KlineChart — OHLC + footprint rendering
│   ├── heatmap/       # HeatmapChart — order flow depth heatmap
│   ├── comparison/    # ComparisonChart — multi-series overlay
│   ├── study_renderer/ # Renders StudyOutput primitives to canvas (line, band, bar, histogram, profile)
│   ├── overlay/       # Crosshair, ruler, last price, gap markers
│   ├── drawing/       # Drawing tools (lines, boxes) with persistence
│   ├── scale/         # Axis scaling — linear & timeseries
│   └── perf/          # LOD (level-of-detail) rendering optimization
├── components/        # Reusable UI component library
│   ├── display/       # Toast, tooltip, status dot, progress bar, empty state, key-value
│   ├── input/         # Text, numeric, checkbox, dropdown, color picker, slider, toggle, search, secure, multi-select
│   ├── layout/        # Card, collapsible, multi-split, reorderable list, toolbar, button grid/group
│   ├── overlay/       # Confirm dialog, context menu, dropdown menu, form modal, modal shell
│   ├── form/          # Form field, form row, form section
│   └── primitives/    # Icon button, icons, badge, label, separator, truncated text
├── modals/            # Application & pane-level modals
│   ├── pane/          # Pane modals: tickers, calendar, indicators, stream, settings (kline/heatmap/comparison/panel/study)
│   ├── connections/   # Data feed connection status
│   ├── data_feeds/    # Feed management & preview
│   ├── download/      # Historical data download & data management
│   ├── drawing_tools/ # Drawing tool selection panel
│   ├── layout/        # Layout manager (save/load/switch layouts)
│   ├── replay/        # Replay playback controller
│   └── theme/         # Theme editor
├── style/             # Theming: tokens, palette, button/container/canvas/widget styles
├── platform.rs        # data_path(), open_data_folder() — platform I/O (not in data crate)
├── secrets.rs         # SecretsManager — API key storage via OS keyring (not in data crate)
├── layout.rs          # Layout & Dashboard serialization, LayoutId
├── window.rs          # Multi-window management, WindowSpec, popout support
├── error.rs           # InternalError (Chart, Data, Rendering variants)
└── logger.rs          # Async file logging with rotation (50MB max)

crates/data/            # Data layer — kairos-data (pure business logic, no I/O)
├── domain/            # Core types: Price, Trade, Candle, DepthSnapshot, Options, Futures
│   ├── error.rs       # ErrorSeverity enum, AppError trait (user_message, is_retriable, severity)
│   ├── types.rs       # Value objects: Price (i64, 10^-8 precision), Volume, Timestamp, Side
│   ├── entities.rs    # Trade, Candle, DepthSnapshot, MarketData
│   ├── chart/         # ChartConfig, ChartData, ChartBasis, ChartType, LoadingStatus (split module)
│   ├── futures.rs     # FuturesTicker, FuturesTickerInfo, ContractSpec, Timeframe
│   ├── options.rs     # OptionContract, OptionChain, OptionSnapshot
│   ├── panel/         # Panel types: depth grouping, trade aggregation, chase tracking
│   └── aggregation.rs # Trade-to-candle aggregation logic
├── repository/        # Async trait definitions: TradeRepository, DepthRepository, Option*Repository
├── services/          # MarketDataService, OptionsDataService, GexCalculationService, ReplayEngine, FeedMerger, CacheManager
├── state/             # Persistence & state types
│   ├── app.rs         # AppState (persisted): layout manager, theme, timezone, feed configs, downloaded tickers registry
│   ├── chart.rs       # ChartState (in-memory only): config, data, loading status
│   ├── layout.rs      # Layout, Dashboard, LayoutManager types
│   ├── pane.rs        # Pane configuration (serializable)
│   ├── registry.rs    # DownloadedTickersRegistry — tracks downloaded ticker date ranges
│   ├── replay.rs      # ReplayState, PlaybackStatus, SpeedPreset
│   └── persistence.rs # Versioned serialization & migrations (load/save take base_dir from caller)
├── config/            # Theme, timezone, sidebar, panel configuration; config/secrets: ApiProvider, ApiKeyStatus (domain only)
├── feed/              # DataFeedManager, FeedConfig, FeedKind (Databento/Rithmic)
├── drawing/           # Drawing entity types (SerializableColor etc.)
├── error.rs           # DataError
└── util/              # Formatting, time, math, logging helpers

crates/exchange/        # Exchange layer — kairos-exchange (adapters & repository impls)
├── adapter/
│   ├── databento/     # CME Globex historical futures — Databento API (.dbn.zst cache)
│   ├── rithmic/       # CME Globex real-time streaming — Rithmic (rithmic-rs)
│   ├── massive/       # US options — Polygon Massive API
│   ├── error.rs       # AdapterError (fetch, parse, connection, invalid request)
│   ├── event.rs       # Event enum: historical + live events (depth, kline, trade, connect/disconnect)
│   └── stream.rs      # StreamKind, PersistStreamKind, ResolvedStream, UniqueStreams
├── repository/
│   ├── databento/     # DatabentoTradeRepository, DatabentoDepthRepository
│   ├── rithmic/       # RithmicTradeRepository, RithmicDepthRepository
│   └── massive/       # MassiveChainRepository, MassiveContractRepository, MassiveSnapshotRepository
└── error.rs           # Error enum with UserFacingError trait

crates/study/           # Study layer — kairos-study (technical analysis library)
├── traits.rs          # Study trait, StudyCategory, StudyPlacement, StudyInput
├── output.rs          # StudyOutput: Lines, Band, Bars, Histogram, Levels, Profile, Clusters
├── config.rs          # ParameterDef, ParameterValue, StudyConfig
├── registry.rs        # StudyRegistry — factory for 15 built-in studies
├── volume/            # Volume, Delta, CVD, OBV
├── trend/             # SMA, EMA, VWAP
├── momentum/          # RSI, MACD, Stochastic
├── volatility/        # ATR, Bollinger Bands
└── orderflow/         # Volume Profile, POC, Value Area, Imbalance
```

## Key Patterns

**Elm Architecture (Iced)**: `Kairos` struct implements `new()`, `update(Message) -> Task<Message>`, `view()`, `subscription()`. Messages route hierarchically: top-level `Message` → `dashboard::Message` → `pane::Message` → `chart::Message`.

**Hierarchical Message Routing**: Each layer handles its own message domain. `app/src/app/update/` splits handlers by concern (chart, download, feeds, navigation, options, preferences, replay).

**Generic Chart Trait**: `Chart` trait in `app/src/chart/core/traits.rs` provides a unified interface. `KlineChart`, `HeatmapChart`, and `ComparisonChart` all implement it. Chart update/view logic is generic over `T: Chart`.

**Pane Content Polymorphism**: `Content` enum (`app/src/screen/dashboard/pane/content.rs`) holds `Starter`, `Kline`, `Heatmap`, `TimeAndSales`, `Ladder`, or `Comparison`. Panes can switch content types without losing layout position.

**Repository Pattern**: Async traits defined in `crates/data/repository/traits.rs`, implemented in `crates/exchange/repository/`. Services depend on traits, not concrete adapters.

**Multi-Window Popouts**: Dashboard tracks `popout: HashMap<window::Id, (PaneGridState, WindowSpec)>`. Panes pop out to separate OS windows with persisted positions.

**Study System**: `crates/study/` crate provides trait-based technical analysis. Studies implement `Study` trait → `compute(StudyInput)` → `StudyOutput`. The `StudyRegistry` factory creates instances by ID. `app/src/chart/study_renderer/` converts `StudyOutput` to canvas draw calls.

**Stream Subscriptions**: Two-tier model — `PersistStreamKind` (serializable config) → resolved at runtime to `StreamKind` (with full `FuturesTickerInfo`). `UniqueStreams` deduplicates across panes.

**Global Event Staging**: `OnceLock<Arc<Mutex<>>>` globals in `app/src/app/globals.rs` (`DOWNLOAD_PROGRESS`, `RITHMIC_EVENTS`, `REPLAY_EVENTS`) stage non-Clone events for the Elm architecture.

**Error Hierarchy**: All error types implement `user_message()`, `is_retriable()`, `severity()` via `AppError` trait (data layer) and `UserFacingError` trait (exchange layer). Use `thiserror` for derivation.

**Fixed-Point Arithmetic**: `Price` type = i64 with 10^-8 precision. Never use floating point for price values.

**Per-Day Caching**: Historical data cached by date in `cache/databento/` (.dbn.zst) and `cache/massive/` (.zst). Repositories check cache first, fetch only missing date ranges.

**Service Threading**: Services wrapped in `Arc<Mutex<>>` or `Arc<tokio::sync::Mutex<>>` for async sharing. Created in `app/src/app/services.rs`.

## Code Style

- Max line width: 100 characters (rustfmt.toml)
- Clippy: max 16 function arguments, 5 enum variant names (clippy.toml)
- Rust edition 2024
- Use `thiserror` for error types with `user_message()`, `is_retriable()`, `severity()` methods
- Non-blocking I/O via `Task::perform` — never block the UI thread

## Supported Instruments

- **Futures**: ES, NQ, YM, RTY, ZN, ZB, ZF, GC, SI, NG, HG, CL (CME Globex via Databento + Rithmic)
- **Options**: US-listed equity options (via Polygon Massive API)
