# Kairos Agent Documentation

This document provides detailed information for AI agents working with Kairos, particularly about the headless CLI, data handling, and backtesting functionality.

## Table of Contents

1. [Headless CLI](#headless-cli)
2. [Databento DBN File Handling](#databento-dbn-file-handling)
3. [Calendar Spread Filtering](#calendar-spread-filtering)
4. [Backtest Configuration](#backtest-configuration)
5. [Building and Testing](#building-and-testing)
6. [Architecture Overview](#architecture-overview)

---

## Headless CLI

The Kairos CLI provides headless backtesting functionality without requiring the GUI.

### Building

```bash
cargo build --package kairos-cli
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
| `--strategy` | No | `orb` | Strategy ID |
| `--timeframe` | No | `1min` | Candle timeframe (1m, 5m, 15m, 1h, 1d) |
| `--capital` | No | `100000` | Initial capital in USD |
| `--data-dir` | Yes | - | Directory containing DBN files |
| `--verbose` | No | false | Show detailed trade output |
| `--format` | No | `text` | Output format (text, json) |

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

### Strategy Parameters

Strategies can be configured with parameters:

```rust
// ORB strategy parameters
ParameterDef {
    key: "or_minutes".into(),      // Opening range duration
    default: ParameterValue::Integer(30),
}
ParameterDef {
    key: "tp_multiple".into(),      // Take-profit distance
    default: ParameterValue::Float(1.5),
}
```

---

## Building and Testing

### Build Commands

```bash
# Full build
cargo build --release

# CLI only
cargo build --package kairos-cli

# With gcc wrapper (sandbox environments)
PATH="/tmp/cargo-bin:$PATH" CARGO_HOME="$PWD/.cargo" cargo build --package kairos-cli
```

### Test Commands

```bash
# All tests
cargo test

# Specific packages
cargo test --package kairos-data
cargo test --package kairos-study
cargo test --package kairos-backtest

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
PATH="/tmp/cargo-bin:$PATH" cargo build --package kairos-cli
```

---

## Architecture Overview

### Crate Structure

```
kairos/
├── app/                    # Iced GUI application
├── crates/
│   ├── cli/               # Headless CLI (NEW)
│   │   └── src/
│   │       ├── main.rs    # CLI entry point
│   │       ├── backtest.rs # Backtest command + DBN file provider
│   │       └── download.rs # Download command
│   ├── data/              # Data layer
│   │   └── src/adapter/databento/
│   │       ├── decoder.rs  # DBN decoding
│   │       └── mapper.rs   # Type conversion
│   ├── study/             # Technical analysis
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

The CLI implements `DbnFileProvider` for local DBN files:

```rust
impl TradeProvider for DbnFileProvider {
    fn get_trades(&self, ticker: &FuturesTicker, range: &DateRange) -> ... {
        // 1. Find matching DBN files
        // 2. Filter by instrument ID (exclude spreads)
        // 3. Convert prices from 10^-9 to 10^-8 precision
        // 4. Return sorted trades
    }
}
```

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
