# CLAUDE.md

Kairos is a native desktop charting platform for futures markets built with Rust and Iced (v0.14). Provides real-time and historical market data visualization: candlestick/footprint/profile charts, heatmaps, order flow analysis, comparison charts, AI assistant, and backtesting. Supports multi-window layouts with popout panes.

## Build & Test

```bash
cargo build                          # Dev build
cargo build --release                # Release build
cargo run --release                  # Run app
cargo test                           # All tests
cargo test --package kairos-data
cargo test --package kairos-study
cargo test --package kairos-backtest
cargo clippy --features heatmap -- -D warnings  # Lint (match CI)
cargo fmt --check                    # Format check
```

**Important**: CI runs clippy with `--features heatmap -- -D warnings`. Always lint with features enabled locally to catch feature-gated issues before pushing.

## Environment Variables

```bash
DATABENTO_API_KEY=your_key           # Required for historical futures data
RUST_LOG=kairos_data=debug           # Logging level
```

Rithmic credentials are managed via `keyring` (OS credential store), configured through the UI.

## Architecture

Four workspace crates + one app (Rust edition 2024, all v1.0.0):

```
app/src/                     # Application layer — kairos v1.0.0 (Iced GUI)
├── main.rs                  # Entry point, daemon setup
├── app/                     # Kairos struct, Elm architecture orchestration
│   ├── mod.rs               # Kairos struct, new(), re-exports
│   ├── messages.rs          # Message, ChartMessage, DownloadMessage, WindowMessage, BacktestMessage
│   ├── state/               # Domain-grouped state structs
│   │   ├── ui.rs            # UiState: theme, sidebar, timezone, preferences, notifications
│   │   ├── services.rs      # DataEngineState: DataEngine, RithmicClient, ReplayEngine
│   │   ├── connections.rs   # ConnectionState: ConnectionManager
│   │   ├── persistence.rs   # PersistenceState: layouts, registry, data index, tickers
│   │   └── modals.rs        # ModalState: all overlay/panel state, backtest subsystem
│   ├── backtest/            # Backtest app state (BacktestHistory, BacktestStatus)
│   ├── core/                # App-level globals and subscriptions
│   │   ├── globals.rs       # EventChannel<T>, DATA_ENGINE_EVENT_SLOT, RITHMIC_CLIENT_STAGING, REPLAY/BACKTEST/AI channels
│   │   └── subscriptions.rs # build_subscription(), data_event_monitor, replay/backtest/ai stream monitors
│   ├── init/                # Startup and service creation
│   │   ├── bootstrap.rs     # seed_data_index_from_registry, auto_connect_feeds, handle_services_ready
│   │   ├── services.rs      # initialize_data_engine, DataEngineInit
│   │   └── ticker_registry.rs # FUTURES_PRODUCTS, build_tickers_info
│   ├── layout/              # Layout and dashboard operations
│   │   └── dashboard.rs     # active_dashboard, load_layout, save_state_to_disk
│   ├── update/              # Message handlers
│   │   ├── mod.rs           # Kairos::update() dispatch
│   │   ├── ai/              # AI assistant handlers
│   │   │   ├── mod.rs       # AI message dispatch, drawing bridge orchestration
│   │   │   ├── snapshot.rs  # Chart context snapshots for AI
│   │   │   └── drawing_bridge.rs # Converts ai::DrawingSpec → SerializableDrawing
│   │   ├── backtest.rs      # Backtest run/progress/completion
│   │   ├── chart.rs         # Chart data loading
│   │   ├── data_events.rs   # DataEvent processing
│   │   ├── download.rs      # Historical download handlers
│   │   ├── feeds/           # Data feed connect/disconnect
│   │   │   ├── databento.rs
│   │   │   └── rithmic.rs
│   │   ├── menu_bar.rs      # Menu bar actions
│   │   ├── shell.rs         # Tick, window events, exit, go_back, data folder
│   │   ├── preferences.rs   # User preferences
│   │   └── replay.rs        # Replay control
│   └── view/                # Top-level view
│       ├── main.rs          # Kairos::view() — root view
│       └── sidebar.rs       # Sidebar modal overlay
├── screen/                  # Screen modules
│   ├── dashboard/           # Main dashboard: pane grid, sidebar, ladder
│   │   ├── pane/            # Pane system
│   │   │   ├── types.rs     # Pane struct, AiAssistantState
│   │   │   ├── messages.rs  # Pane message enums
│   │   │   ├── config/      # ContentKind, CandlestickConfig, HeatmapConfig, ComparisonConfig, ProfileConfig, VisualConfig, LinkGroup
│   │   │   ├── content/     # Content enum: Starter, Candlestick, Heatmap*, Ladder*, Comparison, Profile, AiAssistant
│   │   │   ├── ai/          # AI pane state, models, tick actions
│   │   │   ├── drawing/     # Drawing interaction, properties, rebuild
│   │   │   ├── update/      # Pane update: chart, indicators, ai, modal
│   │   │   └── view/        # Pane rendering: header, body, controls, chart views, assistant view
│   │   ├── ladder/          # Depth ladder panel: config, domain (depth grouping, chase tracker, trade aggregator), render
│   │   ├── sidebar.rs       # Dashboard sidebar
│   │   └── update/          # Dashboard update: chart_ops, feed_ops
│   └── backtest/            # Backtest screens
│       ├── launch/          # Launch modal: catalog view, settings view
│       └── manager/         # Results manager: overview, analytics, trades, computed metrics
│           ├── charts/      # Equity, drawdown, histogram, bar, scatter, monte_carlo, returns_grid
│           └── trade_detail/ # Trade detail view: chart (candlestick + markers), sections, strategy_context
├── chart/                   # Chart engine
│   ├── core/                # Chart trait (definition.rs), ViewState, Caches, Interaction (pan/zoom/drawing/ruler)
│   ├── candlestick/         # KlineChart — OHLC + footprint rendering
│   ├── heatmap/             # HeatmapChart — order flow depth heatmap (feature: heatmap)
│   ├── comparison/          # ComparisonChart — multi-series overlay
│   ├── profile/             # ProfileChart — volume profile chart
│   ├── study_renderer/      # StudyOutput → canvas: primitives (line, band, bar, histogram, levels, markers), footprint, vbp
│   ├── overlay/             # Crosshair, ruler, grid, last price, gap markers
│   ├── drawing/             # Drawing tools: lines, shapes, channel, fibonacci, annotations, calculator, volume_profile
│   ├── scale/               # Axis scaling: linear, timeseries, labels, x/y axes
│   ├── shared/              # Shared chart helpers (study_helper)
│   └── perf/                # LOD (level-of-detail) rendering optimization
├── components/              # Reusable UI component library
│   ├── chrome/              # Custom title bar
│   ├── display/             # Toast, tooltip, status dot, progress bar, empty state, key-value
│   ├── input/               # Text, numeric, checkbox, toggle, slider, stepper, search, secure, dropdown, radio, link group
│   ├── layout/              # Card, interactive card, collapsible, multi-split, reorderable list, button group, section header
│   ├── overlay/             # Modal shell, modal header, form modal, confirm dialog, context menu
│   ├── form/                # Form field, form row, form section
│   └── primitives/          # Icon button, icons, badge, label, separator
├── modals/                  # Application & pane-level modals
│   ├── pane/                # Tickers, calendar, indicator manager, stream, settings (candlestick/heatmap/comparison/profile/panel/study)
│   ├── connections/         # Data feed connection status
│   ├── data_feeds/          # Feed management & preview
│   ├── download/            # Historical data download, data management, API key setup
│   ├── drawing/             # Drawing tool selection, properties (fibonacci, calculator, vbp)
│   ├── replay/              # Replay setup, controller, volume trackbar
│   ├── cache_management/    # Cache management modal
│   └── theme/               # Theme editor
├── style/                   # Theming system
│   ├── theme/               # Palette, Iced theme conversion
│   ├── tokens/              # Typography, spacing, border, shadow, alpha, layout, chart, component, calendar, backtest
│   └── widgets/             # Button, container, canvas, slider styles
├── persistence/             # State persistence: AppState, WindowSpec, AiPreferences, Layout, LayoutManager, load/save
├── config/                  # App config: Theme, UserTimezone, Sidebar, ScaleFactor, secrets (ApiProvider, ApiKeyStatus)
├── infra/                   # Infrastructure: logger (async file rotation), platform (data_path), secrets (OS keyring)
├── drawing/                 # Drawing entity types (DrawingTool, SerializableDrawing)
├── services/                # ReplayEngine, ReplayEvent, TradeProvider
└── window.rs                # Multi-window management, WindowSpec, popout support

crates/ai/                   # AI layer — kairos-ai v1.0.0 (GUI-independent AI logic)
├── client/                  # streaming.rs (OpenRouter agentic), config.rs (models)
├── domain/                  # messages.rs, snapshot.rs, conversation.rs
├── tools/                   # 14 tools: market_data, trades, studies, analysis, drawing
├── event.rs                 # AiStreamEvent, DrawingAction, DrawingSpec
└── prompt.rs                # build_system_prompt(), TimezoneResolver trait

crates/data/                 # Data layer — kairos-data v1.0.0 (domain types + adapters)
├── domain/                  # Pure value objects and entities — no I/O
│   ├── core/                # Price (i64, 10^-8), PriceStep, PriceExt, FeedId, Timestamp, Volume, Side, SerializableColor
│   ├── instrument/          # FuturesTicker, FuturesTickerInfo, ContractSpec, Timeframe
│   ├── market/              # Trade, Candle, Depth (BTreeMap<i64, f32>)
│   ├── chart/               # ChartConfig, ChartData, ChartBasis, ChartType, ViewConfig, Autoscale, LoadingStatus, KlineDataPoint, HeatmapIndicator
│   ├── data/                # DataIndex, DownloadedTickersRegistry
│   └── replay/              # ReplayState, PlaybackStatus
├── adapter/                 # Exchange adapters (feature-gated)
│   ├── databento/           # CME Globex historical — Databento API (client, decoder, mapper, symbology, fetcher)
│   └── rithmic/             # CME Globex real-time — R|Protocol WebSockets (client, plants, protocol, streaming, pool)
├── aggregation/             # Trade-to-candle: time-based, tick-based, re-aggregation
├── cache/                   # Per-day file caching: bincode + zstd (store, format, live_buffer, stats)
├── connection/              # ConnectionManager, Connection, ConnectionProvider, ConnectionStatus, ConnectionCapability, config types
├── engine/                  # DataEngine facade: routes requests, manages cache, emits DataEvents via mpsc
├── stream/                  # PersistStreamKind, StreamKind, ResolvedStream, UniqueStreams, DownloadSchema
├── event.rs                 # DataEvent enum (connection lifecycle, market data, downloads, data index)
├── error.rs                 # Error enum (Fetch, Config, Cache, Symbol, Connection, Validation, NoData, Aggregation, Io) with AppError trait
└── util/                    # Formatting, time, math, serde, logging helpers

crates/study/                # Study layer — kairos-study v1.0.0 (technical analysis library, pure computation)
├── core/                    # Study trait (14 methods, Send+Sync), StudyInput, StudyCategory, StudyPlacement
├── config/                  # ParameterDef, ParameterKind, ParameterValue, StudyConfig, DisplayFormat, Visibility
├── output/                  # StudyOutput: Lines, Band, Bars, Histogram, Levels, Profile, Footprint, Markers, Composite, Empty
│   ├── series.rs            # LineSeries, BarSeries, BarPoint, HistogramBar, PriceLevel
│   ├── markers.rs           # TradeMarker, MarkerData, MarkerRenderConfig
│   ├── footprint/           # FootprintData, FootprintCandle, FootprintLevel, FootprintScaling
│   └── profile/             # ProfileOutput, ProfileLevel, VolumeNode, VBP config types
├── studies/                 # 16 built-in studies
│   ├── registry.rs          # StudyRegistry factory
│   ├── volume/              # Volume, Delta, CVD, OBV
│   ├── trend/               # SMA, EMA, VWAP
│   ├── momentum/            # RSI, MACD, Stochastic
│   ├── volatility/          # ATR, Bollinger Bands
│   └── orderflow/           # Footprint, VBP, Big Trades, Imbalance
├── error.rs                 # StudyError with AppError impl
└── util/                    # candle helpers (source_value, candle_key), math (mean, variance, std_dev)

crates/backtest/             # Backtest layer — kairos-backtest v1.0.0 (event-driven strategy simulation)
├── config/                  # BacktestConfig, InstrumentSpec, RiskConfig, MarginConfig, SlippageModel
├── engine/                  # Engine kernel (simulation loop), BacktestRunner, StrategyContext, EngineClock, SessionClock
├── feed/                    # TradeProvider trait, DataFeed (multi-stream merge), CandleAggregator, MultiTimeframeAggregator
├── fill/                    # FillSimulator trait, StandardFillSimulator, DepthBasedFillSimulator, LatencyModel
├── order/                   # Order, OrderBook, OrderRequest (Submit/Bracket/Cancel/Modify/Flatten), OrderType, TimeInForce
├── portfolio/               # Portfolio (cash, positions, margin), Position (VWAP, MAE/MFE), EquityCurve, accounting
├── strategy/                # Strategy trait, StrategyContext, StudyBank, StrategyRegistry
│   └── built_in/            # ORB, VWAP Reversion, Momentum Breakout
├── output/                  # BacktestResult, PerformanceMetrics, TradeRecord, TradeSnapshot, ExitReason, BacktestProgressEvent
├── analysis/                # t-test, bootstrap CI, Monte Carlo simulation
└── optimization/            # WalkForwardOptimizer, ParameterGrid, ObjectiveFunction
```

## Kairos Struct

```rust
pub struct Kairos {
    pub(crate) main_window: window::Window,
    pub(crate) menu_bar: MenuBar,
    pub(crate) ui: state::UiState,              // theme, timezone, sidebar, preferences, notifications
    pub(crate) services: state::DataEngineState, // DataEngine, RithmicClient, ReplayEngine
    pub(crate) connections: state::ConnectionState, // ConnectionManager
    pub(crate) persistence: state::PersistenceState, // layouts, registry, data index, tickers_info
    pub(crate) modals: state::ModalState,        // all overlay/panel state, backtest subsystem
    pub(crate) secrets: SecretsManager,
}
```

## Key Patterns

**Elm Architecture (Iced)**: `Kairos` implements `new()`, `update(Message) -> Task<Message>`, `view()`, `subscription()`. Messages route hierarchically: top-level `Message` -> `dashboard::Message` -> `pane::Message`.

**Hierarchical Message Routing**: `app/src/app/update/` splits handlers by concern (ai, backtest, chart, data_events, download, feeds, menu_bar, preferences, replay, shell).

**Pane Content Polymorphism**: `Content` enum holds `Starter`, `Candlestick`, `Heatmap` (feature-gated), `Ladder` (feature-gated), `Comparison`, `Profile`, or `AiAssistant`. Panes can switch content types without losing layout position.

**Chart System**: Chart trait in `app/src/chart/core/definition.rs` provides unified interface. `KlineChart`, `HeatmapChart`, `ComparisonChart`, and `ProfileChart` implement it. `app/src/chart/study_renderer/` converts `StudyOutput` to canvas draw calls.

**DataEngine Facade**: `data::engine::DataEngine` is the unified data access layer. Routes requests to adapters (Databento, Rithmic), manages the shared `CacheStore`, maintains a `DataIndex`, and emits `DataEvent`s through an mpsc channel. No separate exchange crate.

**Event Channel System**: `EventChannel<T>` in `app/src/app/core/globals.rs` provides `OnceLock<(UnboundedSender, Mutex<Option<UnboundedReceiver>>)>` pairs. Channels exist for: `DataEvent` (via `DATA_ENGINE_EVENT_SLOT`), `ReplayEvent`, `BacktestProgressEvent`, `AiStreamEventClone`. Subscriptions in `core/subscriptions.rs` drain these via polling.

**Stream Subscriptions**: Two-tier model — `PersistStreamKind` (serializable config with ticker only) -> resolved at runtime to `StreamKind` (with full `FuturesTickerInfo`). `UniqueStreams` deduplicates across panes.

**Study System**: `crates/study/` provides trait-based technical analysis. 16 built-in studies implement `Study` trait -> `compute(StudyInput)` -> `StudyOutput`. `StudyRegistry` factory creates instances by ID.

**Error Hierarchy**: All error types implement `user_message()`, `is_retriable()`, `severity()` via `AppError` trait. `data::Error` has 9 variants (Fetch, Config, Cache, Symbol, Connection, Validation, NoData, Aggregation, Io). Use `thiserror` for derivation.

**Fixed-Point Arithmetic**: `Price` type = i64 with 10^-8 precision. Never use floating point for price values. `Depth.bids`/`asks` uses `BTreeMap<i64, f32>` where keys are raw price units.

**Per-Day Caching**: Historical data cached by date in `{cache_root}/{provider}/{symbol}/{schema}/{date}.bin.zst`. Bincode + zstd (level 3). Atomic writes via `.tmp` + rename.

**Multi-Window Popouts**: Dashboard tracks popout panes in separate OS windows with persisted positions.

**AI Assistant**: Full conversational AI pane with streaming responses, tool use (market data, studies, drawings, trades, analysis), and chart context snapshots.

## Code Style

- Max line width: 100 characters (rustfmt.toml)
- Clippy: max 16 function arguments, 5 enum variant names (clippy.toml)
- Rust edition 2024
- Use `thiserror` for error types with `user_message()`, `is_retriable()`, `severity()` methods
- Non-blocking I/O via `Task::perform` — never block the UI thread

## Feature Flags

- `heatmap` — Enables depth-based heatmap chart, ladder panel, and `Depth` variants in `DataEvent`/`StreamKind`
- `options` — Enables options-related UI (currently placeholder)
- `debug` — Enables Iced hot-reloading

## CI/CD

GitLab CI (`.gitlab-ci.yml`) — push/MR/tag pipelines:
- **check**: `fmt`, `clippy --features heatmap -- -D warnings`, `audit`
- **test**: `test --features heatmap`, `test --doc --features heatmap`
- **build**: Linux (x86_64, aarch64), Windows (x86_64, aarch64), macOS (universal) — tag-triggered or manual
- **release**: Package checksums + GitLab release with download links

## Supported Instruments

- **Futures**: ES, NQ, YM, RTY, ZN, ZB, ZF, GC, SI, NG, HG, CL (CME Globex via Databento + Rithmic)
