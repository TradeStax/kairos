# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Flowsurface is a native desktop charting platform for futures markets built with Rust and Iced GUI framework. It provides real-time market data visualization with support for candlestick charts, heatmaps, order flow analysis, and options data.

## Build Commands

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run the application
cargo run --release

# Platform-specific builds
./scripts/build-macos.sh [x86_64|aarch64|universal]
./scripts/build-windows.sh
./scripts/package-linux.sh
```

## Testing & Linting

```bash
# Run all tests
cargo test

# Test specific package
cargo test --package flowsurface-data
cargo test --package flowsurface-exchange

# Integration tests (require API keys)
DATABENTO_API_KEY=your_key cargo test --package flowsurface-exchange -- --ignored

# Clippy linting
cargo clippy

# Format check
cargo fmt --check
```

## Environment Variables

```bash
DATABENTO_API_KEY=your_api_key      # Required for futures data
MASSIVE_API_KEY=your_polygon_key     # Optional for options data
FLOWSURFACE_PROFILE=production       # or staging/development
RUST_LOG=flowsurface_data=debug      # Logging level
```

## Architecture

### Three-Layer Structure

```
src/                    # Application layer (Iced GUI)
├── app/               # App state, message handling, services
├── screen/            # Views (dashboard, panes, panels)
├── chart/             # Charting system (candlestick, heatmap, overlays)
├── widget/            # Custom Iced widgets
├── modal/             # Modal dialogs
└── style/             # Theming and styling

data/                   # Data layer (pure business logic, no I/O)
├── domain/            # Core types: Price, Trade, Candle, Options
├── repository/        # Trait definitions for data access
├── services/          # MarketDataService, OptionsDataService, GexCalculationService
└── state/             # State persistence and migrations

exchange/               # Exchange layer (adapters)
├── adapter/
│   ├── databento/     # CME Globex futures via Databento API
│   └── massive/       # US options via Polygon API
└── repository/        # Repository implementations
```

### Key Patterns

**Elm Architecture**: The app uses Iced's Elm-inspired pattern:
- `Message` enum defines all events
- `update()` handles messages and returns `Task<Message>`
- `view()` renders the UI
- `subscription()` provides async event streams

**Repository Pattern**: Data access is abstracted via async traits in `data/repository/`, with implementations in `exchange/repository/`.

**Service Initialization**: Services are created in `src/app/services.rs` and wrapped in `Arc<Mutex<>>` for thread-safe sharing.

**Fixed-Point Arithmetic**: `Price` type uses i64 units with 10^-8 precision for accuracy.

**Per-Day Caching**: Trade data is cached by date in `cache/databento/` and `cache/massive/` directories using `.dbn.zst` format.

### Chart System

Located in `src/chart/`, uses a modular architecture:
- `core/` - Unified chart engine with LOD-based rendering
- `candlestick/`, `heatmap/`, `comparison/` - Chart types
- `overlay/` - Crosshair, ruler, price lines
- `indicator/` - Technical indicators
- `scale/` - Axis scaling and labels

### Async Tasks

Non-blocking operations use `Task::perform`:
```rust
Task::perform(
    async move { service.get_chart_data(&config).await },
    |result| Message::ChartDataLoaded { result }
)
```

## Code Style

- Max line width: 100 characters (rustfmt.toml)
- Clippy: max 16 function arguments, 5 enum variants
- Error types include `user_message()`, `is_retriable()`, `severity()` methods
- Use `thiserror` for custom error types

## Supported Instruments

- **Futures**: ES, NQ, YM, RTY, ZN, GC, CL (CME Globex via Databento)
- **Options**: US-listed equity options (via Polygon Massive API)
