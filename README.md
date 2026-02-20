# Kairos

A native desktop charting platform for futures markets built with Rust and [Iced](https://github.com/iced-rs/iced) (v0.14).

![Kairos Header](./assets/illustrations/header.svg)

## Features

- **Candlestick & footprint charts** — OHLC with order-flow footprint and built-in studies (SMA, EMA, RSI, MACD, Bollinger, etc.)
- **Heatmaps** — Order flow depth heatmaps with volume profile and trade markers
- **Comparison charts** — Multi-series overlay for spread or ratio analysis
- **Real-time and historical data** — CME Globex via Databento (historical) and Rithmic (live)
- **Multi-window layouts** — Popout panes, saved layouts, link groups for synchronized tickers
- **Replay** — Replay historical sessions with jump, seek, and speed control
- **JavaScript indicators** — Custom indicators via QuickJS (see [script/README.md](script/README.md))

## Build & run

```bash
cargo build --release
cargo run --release
```

## Tests

```bash
cargo test
cargo test --package kairos-data
cargo test --package kairos-exchange
cargo test --package kairos-study
cargo clippy
cargo fmt --check
```

## Environment

- `DATABENTO_API_KEY` — Required for historical CME futures data
- `MASSIVE_API_KEY` — Optional for US options (Polygon)
- `KAIROS_DATA_PATH` — Override data directory (default: platform data dir / kairos)
- `RUST_LOG` — Log level (e.g. `kairos_data=debug`)

Rithmic credentials are configured via the app (Settings > Data feeds); the app uses the OS keyring where available.

## Project layout

| Crate        | Role |
|-------------|------|
| `src/`      | GUI application (Iced), app state, chart rendering, modals |
| `data/`     | Domain types, repository traits, services, state persistence |
| `exchange/` | Adapters (Databento, Rithmic, Massive) and repository implementations |
| `study/`    | Technical analysis studies (volume, trend, momentum, volatility, order flow) |
| `script/`   | JavaScript indicator engine (QuickJS), loader, compiler |

See [CLAUDE.md](CLAUDE.md) for detailed architecture and conventions.

## License

GPL-3.0-or-later
