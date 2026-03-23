# Kairos Agent Documentation

This document provides detailed information for AI agents working with Kairos, particularly about the headless CLI, data handling, ML training, and backtesting functionality.

## Table of Contents

1. [Headless CLI](#headless-cli)
2. [Databento DBN File Handling](#databento-dbn-file-handling)
3. [Calendar Spread Filtering](#calendar-spread-filtering)
4. [Backtest Configuration](#backtest-configuration)
5. [ML Strategy (LSTM)](#ml-strategy-lstm)
6. [Building and Testing](#building-and-testing)
7. [Architecture Overview](#architecture-overview)

---

## Headless CLI

The Kairos CLI provides headless backtesting functionality without requiring the GUI.

### Building

```bash
cargo build --package kairos-cli --features kairos-cli/tch
```

The binary is located at `./target/debug/kairos`.

### Commands

#### `backtest`

Run a backtest on local DBN files:

```bash
./target/debug/kairos backtest \
  --symbol NQ \
  --start 2023-01-01 \
  --end 2023-12-31 \
  --strategy orb \
  --data-dir /path/to/dbn/files \
  --capital 100000 \
  --timeframe 1min \
  --verbose
```

**Arguments:**

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `--symbol` | Yes | - | Futures symbol (NQ, ES, YM, RTY, etc.) |
| `--start` | Yes | - | Start date (YYYY-MM-DD) |
| `--end` | Yes | - | End date (YYYY-MM-DD) |
| `--strategy` | No | `orb` | Strategy ID (orb, vwap_reversion, momentum_breakout, ml) |
| `--model-path` | No | - | Path to ML model (.safetensors file) |
| `--strategy-config` | No | - | Path to strategy config JSON |
| `--timeframe` | No | `1min` | Candle timeframe (1m, 5m, 15m, 1h, 1d) |
| `--capital` | No | `100000` | Initial capital in USD |
| `--data-dir` | Yes | - | Directory containing DBN files |
| `--verbose` | No | false | Show detailed trade output |
| `--format` | No | `text` | Output format (text, json) |

#### `ml train`

Train an ML model on market data:

```bash
./target/debug/kairos ml train \
  --config training_config.json \
  --data-dir /data/jbutler/algo-data/nq \
  --output models/nq_lstm_model.safetensors \
  --epochs 50 \
  --start 2021-01-01 \
  --end 2021-03-31 \
  --timeframe 5min \
  --verbose
```

**Arguments:**

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `--config` | Yes | - | Training configuration JSON |
| `--data-dir` | Yes | - | Directory containing DBN files |
| `--output` | Yes | - | Output path for model |
| `--symbol` | No | NQ | Symbol to train on |
| `--start` | Yes | - | Start date (YYYY-MM-DD) |
| `--end` | Yes | - | End date (YYYY-MM-DD) |
| `--timeframe` | No | `1min` | Candle timeframe (1s, 5s, 10s, 30s, 1m, 3m, 5m, 15m, 30m, 1h, 4h, 1d) |
| `--epochs` | No | 50 | Number of epochs |
| `--learning-rate` | No | 0.001 | Learning rate |
| `--batch-size` | No | 256 | Batch size |
| `-v, --verbose` | No | - | Verbose output |

**Timeframe Support:**

| Timeframe | Aliases | Description |
|-----------|---------|-------------|
| `1s` | `1sec` | 1 second bars |
| `5s` | `5sec` | 5 second bars |
| `10s` | `10sec` | 10 second bars |
| `30s` | `30sec` | 30 second bars |
| `1min` | `1m` | 1 minute bars (default) |
| `3min` | `3m` | 3 minute bars |
| `5min` | `5m` | 5 minute bars |
| `15min` | `15m` | 15 minute bars |
| `30min` | `30m` | 30 minute bars |
| `1hour` | `1h` | 1 hour bars |
| `4hour` | `4h` | 4 hour bars |
| `1day` | `1d` | Daily bars |

**Important:** When training on a specific timeframe, use the **same timeframe** during backtesting to ensure feature alignment.

#### `list-strategies`

List all available built-in strategies:

```bash
./target/debug/kairos list-strategies
```

Output:
```
Available Strategies
====================
orb: Opening Range Breakout
   Trades breakouts above/below the first N minutes of the RTH session.

vwap_reversion: VWAP Reversion
   Fades price deviations from VWAP at standard-deviation bands.

momentum_breakout: Momentum Breakout
   Donchian channel breakout with ATR-scaled bracket orders.

ml: LSTM Neural Network Strategy
   ML-based strategy using trained PyTorch models.
   Features: 12 technical indicators (SMA, EMA, RSI, ATR, MACD, BB, VWAP)
```

#### `list-symbols`

List all supported futures symbols with their tick values:

```bash
./target/debug/kairos list-symbols
```

#### `debug-data`

Inspect a DBN file's structure and price data:

```bash
./target/debug/kairos debug-data --path /path/to/file.dbn.zst
```

Useful for debugging data issues.

---

## Databento DBN File Handling

### File Naming Convention

Databento DBN files follow this naming convention:

```
{schema}-{YYYYMMDD}-{YYYYMMDD}.{schema}.dbn.zst
```

Examples:
- `glbx-mdp3-20230101-20230131.trades.dbn.zst` — NQ trades, Jan 2023
- `glbx-mdp3-20230101-20230131.mbp-10.dbn.zst` — NQ depth (MBP-10), Jan 2023

### File Structure

DBN files contain:

1. **Header** — Metadata including schema, dataset, version, symbols
2. **Symbol mappings** — Maps instrument IDs to actual symbols
3. **Data records** — TradeMsg, Mbp10Msg, or OhlcvMsg records

### Price Precision

Databento stores prices with **10^-9 precision** (nanodollars). Kairos converts this to **10^-8 precision** internally:

```rust
// Databento price: 11,123,000,000,000 (10^-9) = $11,123.00
// Kairos Price: 1,112,300,000 (10^-8) = $11,123.00

let kairos_price = (databento_price + databento_price.signum() * 5) / 10;
```

The rounding is banker-style (round half to even).

---

## Calendar Spread Filtering

### The Problem

Databento DBN files for futures may contain **calendar spread** trades alongside outright futures contracts. Calendar spreads are priced as the *difference* between two contract months, not absolute prices.

**Example for NQ:**

| Instrument | Type | Price Range | Instrument ID |
|-----------|------|-------------|---------------|
| NQH3 | Outright futures | $10,750 - $12,520 | 20631 |
| NQM3 | Outright futures | $10,870 - $11,595 | 3522 |
| NQZ3 | Outright futures | $11,000 - $11,600 | 260937 |
| NQH3-NQM3 | Calendar spread | $100 - $125 | 19669 |

When analyzing "outlier" ticks (ticks far from the rolling average), calendar spreads appear as extreme outliers because their prices are ~100x smaller than outright prices.

### The Solution

Kairos filters trades by **instrument ID** to exclude calendar spreads. The CLI maintains a list of valid instrument IDs for each futures product:

```rust
fn get_nq_instrument_ids() -> Vec<u32> {
    vec![
        20631,   // NQH3 (NQ March 2023)
        3522,    // NQM3 (NQ June 2023)
        2130,    // NQU3 (NQ September 2023)
        750,     // NQH4 (NQ March 2024)
        260937,  // NQZ3 (NQ December 2023)
        106364,  // NQZ4 (NQ December 2024)
    ]
}
```

### Identifying Instruments

To see what instruments are in a DBN file:

```bash
./target/debug/kairos debug-data --path /path/to/file.dbn.zst
```

This outputs the instrument mappings from the file metadata, showing which instrument IDs correspond to which contracts.

### Adding New Instrument IDs

When new contract months are added, you may need to update the instrument ID list. To find the correct instrument IDs:

1. Run `debug-data` on a file containing the new month
2. Check the instrument mappings output
3. Add the new instrument ID to the appropriate list in `crates/cli/src/backtest.rs`

---

## Backtest Configuration

### BacktestConfig

The `BacktestConfig` struct controls backtest behavior:

```rust
pub struct BacktestConfig {
    pub ticker: FuturesTicker,           // e.g., NQ.c.0
    pub date_range: DateRange,            // Start and end dates
    pub timeframe: Timeframe,             // Candle aggregation
    pub initial_capital_usd: f64,        // Starting equity
    pub risk: RiskConfig,                // Max drawdown, risk-free rate
    pub slippage: SlippageModel,         // Fill simulation
    pub commission_per_side_usd: f64,    // Commission per contract
    pub timezone_offset_hours: i32,      // Session time interpretation
    pub rth_open_hhmm: u32,             // RTH open (e.g., 930 = 9:30)
    pub rth_close_hhmm: u32,            // RTH close (e.g., 1600 = 16:00)
    pub warm_up_periods: usize,          // Candles before strategy starts
    // ... more fields
}
```

### Instrument Specifications

Each futures product has predefined specifications:

```rust
// From crates/backtest/src/config/instrument.rs
match product {
    "NQ" => (0.25, 20.0, 21_000.0, 19_000.0),  // tick_size, multiplier, initial_margin, maintenance_margin
    "ES" => (0.25, 50.0, 15_900.0, 14_400.0),
    "YM" => (1.0, 5.0, 11_000.0, 10_000.0),
    // ...
}
```

- **Tick size**: Minimum price increment (e.g., $0.25 for NQ/ES)
- **Multiplier**: Dollar value per point (e.g., $20/tick for NQ)
- **Margins**: Initial and maintenance margin requirements

---

## ML Strategy (LSTM)

The ML strategy uses a trained LSTM neural network to generate trading signals based on 12 technical indicators.

### GPU Environment Setup

```bash
export LIBTORCH_USE_PYTORCH=1
export LD_LIBRARY_PATH="/home/administrator/.local/lib/python3.12/site-packages/torch/lib:/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
```

### Training Configuration (`training_config.json`)

```json
{
  "model_type": "lstm",
  "learning_rate": 0.001,
  "batch_size": 256,
  "epochs": 50,
  "optimizer": "Adam",
  "weight_decay": 0.0001,
  "validation_split": 0.2,
  "early_stopping_patience": 10,
  "label_config": {
    "horizon": 5,
    "long_threshold": 0.005,
    "short_threshold": 0.005,
    "warmup_bars": 20
  },
  "lstm_config": {
    "hidden_size": 64,
    "num_layers": 2,
    "dropout": 0.2,
    "bidirectional": false
  },
  "gpu_device": 0
}
```

### ML Strategy Configuration (`ml_strategy_config.json`)

```json
{
  "id": "nq_lstm_strategy",
  "name": "NQ LSTM Strategy",
  "signal_threshold_long": 0.45,
  "signal_threshold_short": 0.45,
  "min_confidence": 0.40,
  "sl_tp": {
    "stop_loss_ticks": 20,
    "take_profit_ticks": 30,
    "use_atr_based": false
  },
  "feature_config": {
    "features": [
      {"study_key": "sma_20", "output_field": "line"},
      {"study_key": "sma_50", "output_field": "line"},
      {"study_key": "ema_12", "output_field": "line"},
      {"study_key": "ema_26", "output_field": "line"},
      {"study_key": "rsi", "output_field": "value"},
      {"study_key": "atr", "output_field": "value"},
      {"study_key": "macd", "output_field": "lines.0"},
      {"study_key": "macd_signal", "output_field": "lines.1"},
      {"study_key": "macd_hist", "output_field": "histogram"},
      {"study_key": "bollinger_upper", "output_field": "band.upper"},
      {"study_key": "bollinger_lower", "output_field": "band.lower"},
      {"study_key": "vwap", "output_field": "value"}
    ],
    "lookback_periods": 20
  }
}
```

### Input Features (12 Technical Indicators)

| Feature | Study ID | Period |
|---------|----------|--------|
| sma_20 | sma | 20 |
| sma_50 | sma | 50 |
| ema_12 | ema | 12 |
| ema_26 | ema | 26 |
| rsi | rsi | 14 |
| atr | atr | 14 |
| macd | macd | 12,26,9 |
| macd_signal | macd | 12,26,9 |
| macd_hist | macd | 12,26,9 |
| bollinger_upper | bollinger | 20,2 |
| bollinger_lower | bollinger | 20,2 |
| vwap | vwap | - |

**Total: 12 features × 20 lookback = 240 input dimensions**

> **⚠️ Timeframe Alignment:** When training a model on a specific timeframe (e.g., `--timeframe 5min`), you must use the **same timeframe** during backtesting. The technical indicators and model inputs are computed per-bar, so mismatched timeframes will produce incorrect signals.

### LSTM Architecture

```
Input Layer:  [batch, lookback=20, features=12]
    ↓
LSTM Layer:  64 hidden units, 2 layers
    ↓
Dense Layer:  hidden → 3 (long/neutral/short)
    ↓
Output:      Class probabilities [P(long), P(neutral), P(short)]
```

### Running ML Backtest

```bash
./target/debug/kairos backtest \
  --symbol NQ \
  --start 2021-03-15 \
  --end 2021-03-20 \
  --strategy ml \
  --model-path models/nq_lstm_v2.safetensors \
  --strategy-config ml_strategy_config.json \
  --timeframe 5min \
  --data-dir /data/jbutler/algo-data/nq \
  --capital 100000 \
  --verbose
```

### Bracket Orders (Stop-Loss/Take-Profit)

The ML strategy supports bracket orders with SL/TP:

| Parameter | Description |
|-----------|-------------|
| `stop_loss_ticks` | Stop-loss distance in ticks |
| `take_profit_ticks` | Take-profit distance in ticks |
| `use_atr_based` | Use ATR multipliers instead of fixed ticks |

---

## Building and Testing

### Build Commands

```bash
# Full build with ML support
cargo build --package kairos-cli --features kairos-cli/tch

# Release build
cargo build --release --package kairos-cli --features kairos-cli/tch

# With gcc wrapper (sandbox environments)
PATH="/tmp/cargo-bin:$PATH" CARGO_HOME="$PWD/.cargo" cargo build --package kairos-cli --features kairos-cli/tch
```

### Test Commands

```bash
# All tests
cargo test

# Specific packages
cargo test --package kairos-data
cargo test --package kairos-study
cargo test --package kairos-backtest
cargo test --package kairos-ml

# Lint (with all features)
cargo clippy --features heatmap -- -D warnings
```

### GCC Wrapper

In sandboxed environments where `/usr/bin/cc` is unavailable, create a wrapper:

```bash
mkdir -p /tmp/cargo-bin
cat > /tmp/cargo-bin/cc << 'EOF'
#!/bin/bash
exec /usr/bin/gcc "$@"
EOF
chmod +x /tmp/cargo-bin/cc
```

Then build with:
```bash
PATH="/tmp/cargo-bin:$PATH" cargo build --package kairos-cli --features kairos-cli/tch
```

---

## Architecture Overview

### Crate Structure

```
kairos/
├── app/                    # Iced GUI application
├── crates/
│   ├── cli/               # Headless CLI
│   │   └── src/
│   │       ├── main.rs    # CLI entry point
│   │       ├── backtest.rs # Backtest command + DBN file provider
│   │       └── ml.rs      # ML training command
│   ├── data/              # Data layer
│   │   └── src/adapter/databento/
│   │       ├── decoder.rs  # DBN decoding
│   │       └── mapper.rs   # Type conversion
│   ├── study/             # Technical analysis (SMA, EMA, RSI, etc.)
│   ├── kairos-ml/         # ML training and inference
│   │   └── src/
│   │       ├── model/     # LSTM model implementation
│   │       ├── training/  # Training loop
│   │       ├── features/  # Feature extraction
│   │       └── strategy/  # ML strategy wrapper
│   └── backtest/           # Backtesting engine
│       └── src/
│           ├── engine/    # Simulation kernel
│           ├── strategy/  # Strategy trait + built-ins
│           ├── portfolio/  # Positions, P&L
│           └── output/    # Metrics, results
```

### TradeProvider Trait

The backtest engine uses a `TradeProvider` trait to abstract data sources:

```rust
pub trait TradeProvider: Send + Sync {
    fn get_trades(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Trade>, String>> + Send + '_>>;
}
```

The CLI implements `DbnFileProvider` for local DBN files.

### Key Data Types

| Type | Description |
|------|-------------|
| `Price` | Fixed-point (i64, 10^-8 precision) |
| `Quantity` | Floating-point wrapper (f64) |
| `Timestamp` | Milliseconds since epoch (u64) |
| `Side` | Buy/Sell enum |
| `Trade` | time, price, quantity, side |
| `DateRange` | start/end NaiveDate |

---

## Troubleshooting

### "No trades found"

1. Check that `--data-dir` points to the correct directory
2. Verify file names match the expected pattern (`*.dbn.zst`)
3. Run `debug-data` to inspect the file contents
4. Check that dates are within the file's date range

### "Max drawdown > 100%"

This usually indicates bad price data. Check:
1. Are calendar spreads being filtered correctly?
2. Are prices within expected ranges for the symbol?
3. Run with `--verbose` to see equity curve range

### "Cannot find libtorch"

Ensure environment variables are set:
```bash
export LIBTORCH_USE_PYTORCH=1
export LD_LIBRARY_PATH="/home/administrator/.local/lib/python3.12/site-packages/torch/lib:/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
```

### Build errors with `edition2024`

Ensure you're using a recent Rust nightly:

```bash
rustup default nightly
rustup update
```

### GCC/C compiler not found

Use the gcc wrapper workaround (see [Building and Testing](#building-and-testing))

---

## Contributing

When adding new features to the CLI:

1. **Add commands** in `crates/cli/src/main.rs`
2. **Implement TradeProvider** for new data sources
3. **Update instrument IDs** when new contract months are added
4. **Add tests** for new functionality
5. **Update documentation** (README.md and this file)
