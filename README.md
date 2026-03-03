<img src=".gitlab/kairos.svg" alt="Header" style="max-width: 100px; width: 100%;" />

A native desktop charting platform for futures markets built with Rust and [Iced](https://github.com/iced-rs/iced) (v0.14).

## Features

- **Candlestick & footprint charts** — OHLC with order-flow footprint and built-in studies (SMA, EMA, RSI, MACD, Bollinger, etc.)
- **Heatmaps** — Order flow depth heatmaps with volume profile and trade markers
- **Comparison charts** — Multi-series overlay for spread or ratio analysis
- **Real-time and historical data** — CME Globex via Databento (historical) and Rithmic (live)
- **Multi-window layouts** — Popout panes, saved layouts, link groups for synchronized tickers
- **Replay** — Replay historical sessions with jump, seek, and speed control
- **AI assistant** — Conversational AI pane with tool use for market data, studies, drawings, and analysis
- **Backtesting** — Event-driven strategy simulation with walk-forward optimization and Monte Carlo analysis

## Build & run

```bash
cargo build --release
cargo run --release
```

## Tests

```bash
cargo test
cargo test --package kairos-data
cargo test --package kairos-study
cargo test --package kairos-backtest
cargo clippy
cargo fmt --check
```

## Environment

- `KAIROS_DATA_PATH` — Override data directory (default: platform data dir / kairos)
- `RUST_LOG` — Log level (e.g. `kairos_data=debug`)

Rithmic credentials are configured via the app (Settings > Data feeds); the app uses the OS keyring where available.

## Project layout

| Crate              | Role |
|-------------------|------|
| `app/`            | GUI application (Iced), app state, chart rendering, modals |
| `crates/data/`    | Domain types, adapters (Databento, Rithmic), DataEngine, caching |
| `crates/study/`   | Technical analysis studies (volume, trend, momentum, volatility, order flow) |
| `crates/backtest/`| Event-driven strategy simulation, optimization, performance analysis |

See [CLAUDE.md](CLAUDE.md) for detailed architecture and conventions.

## License

GPL-3.0-or-later
