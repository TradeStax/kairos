<div align="center">

  <img src=".gitlab/kairos.svg" alt="Kairos" width="400" />

  <p align="center">
    A native desktop charting platform for futures markets, built with Rust and <a href="https://github.com/iced-rs/iced">Iced</a>.
  </p>

</div>

<p align="center">
  <img src=".gitlab/screenshot.gif" alt="Kairos screenshot" width="900" />
</p>

---

## Features

- **Candlestick & footprint charts** — OHLC with order-flow footprint overlay, 18 built-in technical studies, and a configurable side panel for volume profile
- **Depth heatmap** *(preview)* — Real-time order book depth visualization with trade markers and volume profile
- **Comparison charts** — Multi-series overlay for spread, ratio, or relative performance analysis
- **Volume profile charts** — Session and composite volume-at-price with POC, value area, and peak/valley detection
- **Depth ladder** *(preview)* — Live depth-of-market ladder with chase tracking, trade aggregation, and grouped price levels
- **19 drawing tools** — Lines, Fibonacci, channels, shapes, annotations, position calculators, and AI context selection
- **Real-time and historical data** — CME Globex via Databento (historical) and Rithmic (live streaming)
- **Multi-window layouts** — Popout panes, saved/restored layouts, and link groups for synchronized tickers
- **Replay** — Replay historical sessions with play/pause, speed control, and seek
- **AI assistant** *(preview)* — Conversational AI pane with 25+ tools for market data, studies, drawings, and analysis
- **Backtesting** *(preview)* — Event-driven strategy simulation with walk-forward optimization, Monte Carlo analysis, and 30+ performance metrics

---

## Downloads

Pre-built binaries are available for each release:

| Platform | Architecture | Download |
|----------|-------------|----------|
| Windows  | x86_64      | [kairos-0.9.0-x86_64-pc-windows-msvc.zip](https://gitlab.com/api/v4/projects/77621610/packages/generic/kairos/0.9.0/kairos-0.9.0-x86_64-pc-windows-msvc.zip) |
| macOS    | Universal   | [kairos-0.9.0-universal-apple-darwin.tar.gz](https://gitlab.com/api/v4/projects/77621610/packages/generic/kairos/0.9.0/kairos-0.9.0-universal-apple-darwin.tar.gz) |
| Linux    | x86_64      | [kairos-0.9.0-x86_64-unknown-linux-gnu.tar.gz](https://gitlab.com/api/v4/projects/77621610/packages/generic/kairos/0.9.0/kairos-0.9.0-x86_64-unknown-linux-gnu.tar.gz) |
| Linux    | aarch64     | [kairos-0.9.0-aarch64-unknown-linux-gnu.tar.gz](https://gitlab.com/api/v4/projects/77621610/packages/generic/kairos/0.9.0/kairos-0.9.0-aarch64-unknown-linux-gnu.tar.gz) |

**Checksums**: [SHA256SUMS.txt](https://gitlab.com/api/v4/projects/77621610/packages/generic/kairos/0.9.0/SHA256SUMS.txt)

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
cargo clippy                         # Lint
cargo fmt --check                    # Format check
```

### Environment Variables

| Variable | Description |
|---|---|
| `DATABENTO_API_KEY` | Databento API key for historical futures data |
| `KAIROS_DATA_PATH` | Override data directory (default: platform data dir / kairos) |
| `RUST_LOG` | Log level (e.g. `kairos_data=debug`) |

---

## Data Providers

### Databento — Historical Data

[Databento](https://databento.com) provides historical trade and MBO data for CME Globex futures.

1. Sign up at databento.com and get an API key
2. Set the `DATABENTO_API_KEY` environment variable, or enter it in the app (Settings > API Keys)
3. Use the download manager to fetch historical data by symbol and date range

Data is cached locally as bincode + zstd compressed files, organized by provider/symbol/schema/date.

### Rithmic — Real-Time Data

[Rithmic](https://rfrithmic.com) provides live market data and order book depth via the R|Protocol.

1. Obtain Rithmic credentials from your broker or a direct Rithmic account
2. Configure credentials in the app (Settings > Data Feeds) — passwords are stored securely in your OS keyring
3. Select the Rithmic server environment (e.g. Rithmic Paper, Rithmic 01)

The app connects to Rithmic's ticker, market data, and PnL plants for real-time trade, candle, and depth streaming.

---

## Chart Types

| Type | Description |
|------|-------------|
| **Candlestick** | OHLC candlesticks with optional footprint overlay, studies, and side panel volume profile |
| **Heatmap** | Order book depth heatmap with trade bubbles, volume profile, and configurable color mapping |
| **Comparison** | Multi-series overlay for spread analysis, ratio comparison, or relative performance |
| **Volume Profile** | Session and composite volume-at-price with POC, value area, and peak/valley detection |
| **Ladder** | Live depth-of-market with grouped price levels, chase tracking, and trade aggregation |

---

## Technical Studies

18 built-in studies organized by category. All studies are configurable and support multiple placement modes (overlay, panel, background, candle replace, side panel).

### Volume

| Study | Description |
|-------|-------------|
| Volume | Total volume per candle |
| Volume Delta | Buy minus sell volume per candle |
| Cumulative Volume Delta | Running cumulative sum of buy/sell delta |
| On Balance Volume | Cumulative volume weighted by price direction |

### Order Flow

| Study | Description |
|-------|-------------|
| Footprint | Per-candle bid/ask volume at each price level |
| Volume by Price | Horizontal volume distribution across price levels |
| Imbalance | Highlights price levels with significant buy/sell imbalance |
| Big Trades | Aggregated institutional-scale trade markers |
| Speed of Tape | Trade activity rate per time bucket as mini-candlesticks |
| Level Analyzer | Auto-detects key price levels and monitors real-time interaction |

### Trend

| Study | Description |
|-------|-------------|
| SMA | Simple Moving Average |
| EMA | Exponential Moving Average |
| VWAP | Volume Weighted Average Price with optional standard deviation bands |

### Momentum

| Study | Description |
|-------|-------------|
| RSI | Relative Strength Index — overbought/oversold oscillator |
| MACD | Moving Average Convergence Divergence with signal line and histogram |
| Stochastic | Stochastic Oscillator with %K and %D lines |

### Volatility

| Study | Description |
|-------|-------------|
| ATR | Average True Range (Wilder's smoothing) |
| Bollinger Bands | SMA with configurable standard deviation bands |

---

## Drawing Tools

19 drawing tools across 7 categories.

| Category | Tools |
|----------|-------|
| **Lines** | Line, Ray, Extended Line, Horizontal Line, Vertical Line |
| **Fibonacci** | Fibonacci Retracement, Fibonacci Extension |
| **Channels** | Parallel Channel |
| **Shapes** | Rectangle, Ellipse |
| **Annotations** | Text Label, Price Label, Arrow |
| **Trading** | Buy Calculator, Sell Calculator |
| **Analysis** | Volume Profile, Delta Profile, AI Context |

Position calculators show entry, target, and stop with automatic risk/reward computation. Volume and delta profile drawings compute volume distribution for any user-selected time range. AI context selections send the selected region to the AI assistant for analysis.

---

## Backtesting

Event-driven backtesting engine with tick-level simulation.

### Built-in Strategies

| Strategy | Description |
|----------|-------------|
| **Opening Range Breakout** | Trades breakouts above/below the first N minutes of the RTH session. Configurable OR period, R-multiple targets, and wick filter. |
| **VWAP Reversion** | Mean-reversion entries when price deviates beyond N standard deviations from session VWAP. Optional slope filter to avoid trending markets. |
| **Momentum Breakout** | Donchian channel breakouts with ATR-based stops and trailing exit via shorter-period channel. |

Custom strategies implement the `Strategy` trait with access to candles, studies, and a full order management API.

### Engine Capabilities

- **Event-driven simulation** — processes every trade tick for realistic fill modeling
- **Multi-timeframe aggregation** — access multiple candle timeframes within a single strategy
- **Session-aware clock** — RTH/ETH session tracking with configurable session times
- **Fill simulation** — standard and depth-based fill models with configurable latency and slippage
- **Order types** — market, limit, stop, stop-limit, bracket orders, and cancel/modify
- **Portfolio tracking** — real-time P&L, margin, VWAP position cost, MAE/MFE per trade

### Performance Metrics

30+ metrics computed automatically after each backtest:

- **P&L** — net/gross P&L (USD and ticks), total commissions, return %
- **Win/Loss** — win rate, profit factor, average win/loss, best/worst trade, expectancy
- **Streaks** — largest win streak, largest loss streak
- **Risk** — max drawdown (USD and %), Sharpe ratio, Sortino ratio, Calmar ratio
- **Excursion** — average MAE/MFE in ticks for stop/target optimization
- **Benchmark** — buy-and-hold return, strategy alpha
- **Duration** — average trade duration, total trading days

### Analysis

- **Monte Carlo simulation** — randomized trade resampling for confidence intervals on drawdown and returns
- **Walk-forward optimization** — rolling in-sample/out-of-sample parameter grid search
- **Bootstrap confidence intervals** — statistical significance testing on strategy performance

---

## AI Assistant

Conversational AI pane with streaming responses and 25+ built-in tools:

- **Market data** — chart info, candle data, current market state
- **Trade analysis** — aggregated trades, volume/delta profiles, session statistics
- **Study values** — read values from all active technical studies
- **Level detection** — automatic support/resistance identification via swing points, volume nodes, and round numbers
- **Drawing actions** — add/remove lines, shapes, Fibonacci, annotations, and price levels directly on the chart

The assistant receives a snapshot of the active chart's data and studies, enabling contextual analysis without leaving the platform.

---

## Replay

Replay historical trading sessions with full chart reconstruction:

- Play, pause, and adjust playback speed
- Seek to any point in the session
- All studies and drawings update in real-time during replay
- Useful for session review, pattern study, and strategy development

---

## Project Layout

| Crate | Lines | Description |
|-------|-------|-------------|
| `app/` | ~45K | Iced GUI application — chart rendering, pane system, modals, state management, AI assistant |
| `crates/data/` | ~15K | Domain types, data adapters (Databento, Rithmic), DataEngine facade, per-day file caching |
| `crates/study/` | ~8K | Technical analysis library — 18 studies with pure computation, no I/O dependencies |
| `crates/backtest/` | ~6K | Event-driven backtesting engine — strategies, fill simulation, optimization, performance analysis |

See [CLAUDE.md](CLAUDE.md) for detailed architecture, conventions, and module-level documentation.

---

## License

GPL-3.0-or-later
