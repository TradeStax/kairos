# Kairos

<div align="center">

  <img src=".gitlab/kairos.svg" alt="Kairos" width="400" />

  <p align="center">
    A native desktop charting platform for futures markets, built with Rust and <a href="https://github.com/iced-rs/iced">Iced</a>.
  </p>

</div>

<p align="center">
  <img src=".gitlab/screenshot.gif" alt="Kairos screenshot" width="800" />
</p>

---

## Features

- **Candlestick & footprint charts** — OHLC with order-flow footprint overlay, 18 built-in technical studies, and a configurable side panel for volume profile
- **Depth heatmap** *(preview)* — Real-time order book depth visualization with trade markers and volume profile
- **Comparison charts** — Multi-series overlay for spread, ratio, or relative performance analysis
- **Volume profile charts** — Session and composite volume-at-price with POC, value area, and peak/vality detection
- **Depth ladder** *(preview)* — Live depth-of-market ladder with chase tracking, trade aggregation, and grouped price levels
- **19 drawing tools** — Lines, Fibonacci, channels, shapes, annotations, position calculators, and AI context selection
- **Real-time and historical data** — CME Globex via Databento (historical) and Rithmic (live streaming)
- **Multi-window layouts** — Popout panes, saved/restored layouts, and link groups for synchronized tickers
- **Replay** — Replay historical sessions with play/pause, speed control, and seek
- **AI assistant** *(preview)* — Conversational AI pane with 25+ tools for market data, studies, drawings, and analysis
- **Backtesting** *(preview)* — Event-driven strategy simulation with walk-forward optimization, Monte Carlo analysis, and 30+ performance metrics
- **Headless CLI** — Run backtests from the command line on local Databento DBN files

<div align="center">
  <img src="https://gitlab.com/kreotic/kairos/-/wikis/charts/candlestick-chart.png" alt="Candlestick chart with studies" width="800" />
  <br>
  <sup>Candlestick chart with technical studies and volume panel</sup>
</div>

<div align="center">
  <img src="https://gitlab.com/kreotic/kairos/-/wikis/studies/footprint.png" alt="Footprint chart" width="800" />
  <br>
  <sup>Footprint — per-price-level order flow within each candle</sup>
</div>

<details>
<summary><b>More screenshots</b> — charts, studies, drawing tools, layouts</summary>
<br>

<div align="center">
  <img src="https://gitlab.com/kreotic/kairos/-/wikis/charts/comparison-chart.png" alt="Comparison chart" width="800" />
  <br>
  <sup>Comparison chart — multi-series overlay for spread and relative analysis</sup>
</div>

<div align="center">
  <img src="https://gitlab.com/kreotic/kairos/-/wikis/charts/profile-chart.png" alt="Volume profile chart" width="800" />
  <br>
  <sup>Volume profile chart with POC, value area, and peak/valley detection</sup>
</div>

<div align="center">
  <img src="https://gitlab.com/kreotic/kairos/-/wikis/studies/ivb.png" alt="IVB — Opening Range study" width="800" />
  <br>
  <sup>IVB — statistical opening range projections with conditional filtering</sup>
</div>

<div align="center">
  <img src="https://gitlab.com/kreotic/kairos/-/wikis/studies/volume_by_price.png" alt="Volume by Price" width="800" />
  <br>
  <sup>Volume by Price — horizontal volume distribution with POC and value area</sup>
</div>

<div align="center">
  <img src="https://gitlab.com/kreotic/kairos/-/wikis/studies/big_trades.png" alt="Big Trades" width="800" />
  <br>
  <sup>Big Trades — aggregated trades illustrating interpreted institutional activity</sup>
</div>

<div align="center">
  <img src="https://gitlab.com/kreotic/kairos/-/wikis/drawing-tools.png" alt="Drawing tools" width="800" />
  <br>
  <sup>19 drawing tools — lines, Fibonacci, channels, shapes, annotations, and position calculators</sup>
</div>

<div align="center">
  <img src="https://gitlab.com/kreotic/kairos/-/wikis/layouts.png" alt="Multi-window layouts" width="800" />
  <br>
  <sup>Multi-window layouts with popout panes and link groups</sup>
</div>

</details>

---

## Downloads

Pre-built binaries for Windows, macOS (Universal), and Linux are available on the [latest release](https://gitlab.com/kreotic/kairos/-/releases/permalink/latest) page.

### System Requirements

- Windows x86_64, macOS (Universal), or Linux (x86_64 / aarch64)
- [Databento](https://databento.com) API key for historical data
- Rithmic credentials for live data (optional, configured through the app)

---

## Building from Source

Requires [Rust](https://rustup.rs/) (edition 2024).

```bash
cargo build --release
cargo run --release
```

### Testing

```bash
cargo test                           # All tests
cargo test --package kairos-data     # Data layer
cargo test --package kairos-study    # Study library
cargo test --package kairos-backtest # Backtest engine
cargo test --package kairos-ml       # ML module (requires libtorch)
cargo clippy                         # Lint
cargo fmt --check                    # Format check
```

---

## Headless CLI

Kairos includes a headless CLI for running backtests from the command line on local Databento DBN files without the GUI.

```bash
# Build the CLI
cargo build --package kairos-cli

# Run a backtest
./target/debug/kairos backtest \
  --symbol NQ \
  --start 2023-01-01 \
  --end 2023-12-31 \
  --strategy orb \
  --data-dir /path/to/dbn/files

# List available strategies
./target/debug/kairos list-strategies

# List supported symbols
./target/debug/kairos list-symbols

# Debug data file structure
./target/debug/kairos debug-data --path /path/to/file.dbn.zst
```

See [AGENTS.md](AGENTS.md) for detailed information about:
- Setting up local Databento data files
- Filtering calendar spreads from trade data
- CLI command reference

---

## Data Providers

### Databento — Historical Data

[Databento](https://databento.com) provides historical trade and MBO data for CME Globex futures.

#### GUI Download
1. Sign up at databento.com and get an API key
2. Set the `DATABENTO_API_KEY` environment variable, or enter it in the app (Settings > API Keys)
3. Use the download manager to fetch historical data by symbol and date range

#### Local DBN Files
The headless CLI can process local Databento DBN files directly:

```bash
# DBN files should be named with the Databento naming convention:
# glbx-mdp3-YYYYMMDD-YYYYMMDD.schema.dbn.zst
# Example: glbx-mdp3-20230101-20230131.trades.dbn.zst

./target/debug/kairos backtest \
  --symbol NQ \
  --data-dir /path/to/dbn/files
```

Data is cached locally as bincode + zstd compressed files, organized by provider/symbol/schema/date.

### Handling Calendar Spreads

Databento DBN files for futures may contain **calendar spread** trades in addition to outright futures contracts. Calendar spreads have significantly different price scales (e.g., $100-$200) compared to outright futures (e.g., $11,000-$17,000 for NQ).

Kairos automatically filters out calendar spreads by instrument ID, ensuring only outright futures trades are used for backtesting.

### Rithmic — Real-Time Data

[Rithmic](https://rfrithmic.com) provides live market data and order book depth via the R|Protocol.

1. Obtain Rithmic credentials from your broker or a direct Rithmic account
2. Configure credentials in the app (Settings > Data Feeds) — passwords are stored securely in your OS keyring
3. Select the Rithmic server environment (e.g. Rithmic Paper, Rithmic 01)

The app connects to Rithmic's ticker, market data, and PnL plants for real-time trade, candle, and depth streaming.

---

## Backtesting

Event-driven backtesting engine with tick-level simulation, 30+ performance metrics, walk-forward optimization, and Monte Carlo analysis.

### Built-in Strategies

| Strategy | Description |
|----------|-------------|
| `orb` | Opening Range Breakout — trades breakouts above/below the first N minutes |
| `vwap_reversion` | VWAP Reversion — fades price deviations from VWAP at std-dev bands |
| `momentum_breakout` | Momentum Breakout — Donchian channel breakout with ATR-scaled brackets |
| ML strategies | Load trained PyTorch models for ML-based signal generation |

### ML Strategy Support

The `kairos-ml` crate provides PyTorch-based ML model support for trading strategies:

```bash
# Train a new ML model
./target/debug/kairos ml train \
  --config training_config.json \
  --data-dir /path/to/training/data

# List available models
./target/debug/kairos ml list-models

# Validate a model
./target/debug/kairos ml validate-model \
  --model trained_model.pt \
  --data sample_data.dbn
```

See [`crates/kairos-ml/README.md`](crates/kairos-ml/README.md) for detailed ML strategy documentation.

### CLI Usage

```bash
# Basic backtest
./target/debug/kairos backtest \
  --symbol NQ \
  --start 2023-01-01 \
  --end 2023-12-31 \
  --strategy orb \
  --capital 100000

# With verbose output
./target/debug/kairos backtest \
  --symbol NQ \
  --start 2023-01-01 \
  --end 2023-01-31 \
  --strategy orb \
  --data-dir /path/to/dbn/files \
  --verbose

# Different timeframes
./target/debug/kairos backtest \
  --symbol NQ \
  --start 2023-01-01 \
  --end 2023-12-31 \
  --strategy orb \
  --timeframe 5min
```

### Performance Metrics

- **Return**: Total, annualized, gross/net
- **Risk**: Max drawdown, Sharpe, Sortino, Calmar ratios
- **Trade Stats**: Win rate, profit factor, expectancy, avg win/loss
- **MAE/MFE**: Maximum adverse/favorable excursion per trade

---

## AI Assistant

Conversational AI pane with streaming responses and 25+ built-in tools:

- **Market data** — chart info, candle data, current market state
- **Trade analysis** — aggregated trades, volume/delta profiles, session statistics
- **Study values** — read values from all active technical studies
- **Level detection** — automatic support/resistance identification via swing points, volume nodes, and round numbers
- **Drawing actions** — add/remove lines, shapes, Fibonacci, annotations, and price levels directly on the chart

The assistant receives a snapshot of the active chart's data and studies, enabling contextual analysis without leaving the platform.

<p align="center">
  <img src="https://gitlab.com/kreotic/kairos/-/wikis/ai-assistant.png" alt="AI Assistant pane" width="800" />
</p>

---

## Replay

Replay historical trading sessions with full chart reconstruction — play/pause, adjustable speed, and seek to any point. All studies and drawings update in real-time.

<p align="center">
  <img src="https://gitlab.com/kreotic/kairos/-/wikis/replay.gif" alt="Chart replay with play/pause, speed control, and seek bar" width="800" />
</p>

---

## Project Layout

| Crate | Lines | Description |
|-------|-------|-------------|
| `app/` | ~45K | Iced GUI application — chart rendering, pane system, modals, state management, AI assistant |
| `crates/cli/` | ~1K | Headless CLI — backtest command, data analysis, strategy listing |
| `crates/data/` | ~15K | Domain types, data adapters (Databento, Rithmic), DataEngine facade, per-day file caching |
| `crates/study/` | ~8K | Technical analysis library — 18 studies with pure computation, no I/O dependencies |
| `crates/backtest/` | ~6K | Event-driven backtesting engine — strategies, fill simulation, optimization, performance analysis |
| `crates/kairos-ml/` | ~15K | PyTorch-based ML strategy module — model loading, feature extraction, training pipeline, ML strategy wrapper |

See [CLAUDE.md](CLAUDE.md) for detailed architecture, conventions, and module-level documentation.

---

## Acknowledgments

Built with significant portions of UI layer and architecture code from [FlowSurface](https://github.com/flowsurface-rs/flowsurface), a crypto-based charting tool. This project extends into futures market support.

FlowSurface itself was inspired by:
- [Kraken Desktop](https://www.kraken.com/desktop) (formerly [Cryptowatch](https://blog.kraken.com/product/cryptowatch-to-sunset-kraken-pro-to-integrate-cryptowatch-features)) — the original inspiration that sparked the project
- [Halloy](https://github.com/squidowl/halloy) — foundational code design and project architecture reference
- [iced](https://github.com/iced-rs/iced) — the GUI library that makes all of this possible

## License

GPL-3.0-or-later
