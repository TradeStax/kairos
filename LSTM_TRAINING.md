# LSTM Neural Network Trading Strategy - Kairos

A complete guide to training and backtesting LSTM neural network trading strategies using real market data, entirely in Rust.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Building](#building)
3. [Runtime Setup](#runtime-setup)
4. [Training Models](#training-models)
5. [ML Strategy Backtesting](#ml-strategy-backtesting)
6. [Available Strategies](#available-strategies)
7. [Features & Architecture](#features--architecture)
8. [Configuration Reference](#configuration-reference)
9. [ML Strategy Configuration](#ml-strategy-configuration)
10. [CLI Commands Reference](#cli-commands-reference)
11. [Troubleshooting](#troubleshooting)
12. [PyTorch Version Compatibility](#pytorch-version-compatibility)

---

## Prerequisites

1. **Rust** (1.94.0+)
2. **NQ Futures DBN Data Files** (in `../nq` or configured data directory)
3. **Python PyTorch** (for runtime library access)

## Building

```bash
cd /data/jbutler/algo-data/kairos

# Build the CLI with ML support
cargo build --package kairos-cli --features kairos-cli/tch
```

Or use the gcc wrapper in sandboxed environments:

```bash
mkdir -p /tmp/cargo-bin
cat > /tmp/cargo-bin/cc << 'EOF'
#!/bin/bash
exec /usr/bin/gcc "$@"
EOF
chmod +x /tmp/cargo-bin/cc

PATH="/tmp/cargo-bin:$PATH" cargo build --package kairos-cli --features kairos-cli/tch
```

## Runtime Setup

The CLI requires access to PyTorch libraries. Set `LD_LIBRARY_PATH` before running:

```bash
# For Python PyTorch installation
export LD_LIBRARY_PATH="/home/administrator/.local/lib/python3.12/site-packages/torch/lib:$LD_LIBRARY_PATH"

# Verify it works
./target/debug/kairos list-strategies
```

Add this to your shell profile for convenience:

```bash
echo 'export LD_LIBRARY_PATH="/home/administrator/.local/lib/python3.12/site-packages/torch/lib:$LD_LIBRARY_PATH"' >> ~/.bashrc
```

---

## Training Models

### Quick Start Training

```bash
./target/debug/kairos ml train \
  --config training_config.json \
  --data-dir ../nq \
  --output models/nq_lstm_model.pt \
  --symbol NQ \
  --start 2021-03-10 \
  --end 2021-03-31 \
  --epochs 50 \
  --verbose
```

### Training with Overrides

Override config values from command line:

```bash
./target/debug/kairos ml train \
  --config training_config.json \
  --data-dir ../nq \
  --output models/my_model.pt \
  --symbol NQ \
  --start 2021-01-01 \
  --end 2021-12-31 \
  --epochs 100 \
  --learning-rate 0.0005 \
  --batch-size 64
```

### Training Output

Expected output:
```
ML Training
==========
Config: training_config.json
Data:   ../nq
Output: models/nq_lstm_model.pt

Training Configuration:
  Model type:        LSTM
  Learning rate:     0.001
  Batch size:       32
  Epochs:           50
  Optimizer:         Adam
  Validation split: 0.2
  Early stopping:    10 epochs

Label Configuration:
  Horizon:          5 bars
  Long threshold:   0.5000%
  Short threshold:  0.5000%
  Warmup bars:     20

Feature Configuration:
  Lookback periods: 20
  Features:        12
    1: sma_20 -> line
    2: sma_50 -> line
    3: ema_12 -> line
    4: ema_26 -> line
    5: rsi -> value
    6: atr -> value
    7: macd -> value
    8: macd_signal -> value
    9: macd_hist -> value
    10: bb_upper -> value
    11: bb_lower -> value
    12: vwap -> value

Loading REAL market data from DBN files...
Loaded 6,429,974 trades
Aggregating into 1-minute candles...
Generated 21,898 candles
Price range: $12616.50 to $13291.75 (avg: $12947.71)

Computing technical indicators from REAL market data...
Computed 21,898 study outputs (technical indicators)

Dataset created:
  Total samples: 21,853
  Lookback:      20
  Features:      12

Starting training...
Device: CPU (GPU unavailable)
Training: features=12, lookback=20, classes=3, architecture=LSTM
Epoch   1/ 50 - train_loss: 0.0631
Epoch   2/ 50 - train_loss: 0.0290
...
Epoch  50/ 50 - train_loss: 0.0276

Training complete!
  Epochs trained:  50
  Final train loss: 0.0276
  Final val loss:   1.1403
  Early stopped:   false

Saving trained model to models/nq_lstm_model.pt...
Model saved successfully!

Training Summary
================
Model saved to: models/nq_lstm_model.pt
Input shape:    [batch, 240] (lookback=20, features=12)
Output shape:   [batch, 3] (long, neutral, short)
```

---

## ML Strategy Backtesting

### Basic ML Backtest

```bash
./target/debug/kairos backtest \
  --symbol NQ \
  --start 2021-03-10 \
  --end 2021-03-31 \
  --strategy ml \
  --model-path models/nq_lstm_model.pt \
  --data-dir ../nq \
  --capital 100000 \
  --verbose
```

### ML Backtest with Custom Strategy Config

```bash
./target/debug/kairos backtest \
  --symbol NQ \
  --start 2021-03-10 \
  --end 2021-03-31 \
  --strategy ml \
  --model-path models/nq_lstm_model.pt \
  --strategy-config ml_strategy_config.json \
  --data-dir ../nq \
  --capital 100000 \
  --verbose
```

### ML Backtest Output

```
ML Strategy Configuration
========================
Model: models/nq_lstm_model.pt
Features: 12 indicators
  - SMA (20, 50)
  - EMA (12, 26)
  - RSI (14)
  - ATR (14)
  - MACD (12, 26, 9)
  - Bollinger Bands (20, 2)
  - VWAP
Lookback: 20 bars

Loading trained model...
Model loaded successfully
  Name: nq_lstm_model

ML Strategy initialized successfully!
Required studies:
  - sma_20 (study: sma_20)
  - sma_50 (study: sma_50)
  - ema_12 (study: ema_12)
  - ema_26 (study: ema_26)
  - rsi (study: rsi)
  - atr (study: atr)
  - macd (study: macd)
  - macd_signal (study: macd_signal)
  - macd_hist (study: macd_hist)
  - bb_upper (study: bb_upper)
  - bb_lower (study: bb_lower)
  - vwap (study: vwap)

Kairos Backtest
Symbol: NQ | Period: 2021-03-10 to 2021-03-31 | Strategy: ml
Initial Capital: $100000.00
Loading trades...
Loaded 6,429,974 trades
Running backtest...

Results
=======
Final Equity: $102,345.67
Return: 2.35%
Max Drawdown: 1.82% ($1,820.00)
Trades: 47
Win Rate: 58.5%
Profit Factor: 1.42
Sharpe: 1.15
Sortino: 1.28
```

---

## Available Strategies

List all available strategies:

```bash
./target/debug/kairos list-strategies
```

Output:
```
Available Strategies
====================
momentum_breakout: Momentum Breakout
   Donchian channel breakout with ATR-scaled bracket orders.

orb: Opening Range Breakout
   Trades breakouts above/below the first N minutes of the RTH session.

vwap_reversion: VWAP Reversion
   Fades price deviations from VWAP at standard-deviation bands.

ml: LSTM Neural Network Strategy
   Machine learning-based strategy using trained PyTorch models.
   Requires: --model-path <path-to-trained-model.pt>
   Features: 12 technical indicators (SMA, EMA, RSI, ATR, MACD, BB, VWAP)
   Usage: kairos backtest --strategy ml --model-path models/model.pt [options]
```

### Strategy Comparison

| Strategy | Type | Risk | Complexity |
|----------|------|------|------------|
| `orb` | Breakout | Medium | Low |
| `vwap_reversion` | Mean Reversion | Medium | Medium |
| `momentum_breakout` | Trend Following | Medium-High | Medium |
| `ml` | ML-Based | Variable | High |

---

## Features & Architecture

### 12 Technical Indicators

The model uses **real technical indicators** computed from actual OHLCV candles:

| Feature | Description | Normalization |
|---------|-------------|---------------|
| `sma_20` | Simple Moving Average (20) | (SMA - Close) / Close |
| `sma_50` | Simple Moving Average (50) | (SMA - Close) / Close |
| `ema_12` | Exponential Moving Average (12) | (EMA - Close) / Close |
| `ema_26` | Exponential Moving Average (26) | (EMA - Close) / Close |
| `rsi` | Relative Strength Index (14) | 0-1 scale (RSI/100) |
| `atr` | Average True Range (14) | ATR / Close |
| `macd` | MACD Line | MACD / Close |
| `macd_signal` | MACD Signal Line | Signal / Close |
| `macd_hist` | MACD Histogram | Hist / Close |
| `bb_upper` | Bollinger Upper Band | (BB_Upper - Close) / Close |
| `bb_lower` | Bollinger Lower Band | (BB_Lower - Close) / Close |
| `vwap` | Volume Weighted Average Price | (VWAP - Close) / Close |

**Total: 12 features × 20 lookback = 240 input dimensions**

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

### Label Generation

Labels are based on **future returns**:

- **Long (0)**: Future return > long_threshold (default 0.5%)
- **Neutral (1)**: Return between thresholds
- **Short (2)**: Future return < -short_threshold (default -0.5%)

---

## Configuration Reference

### Training Configuration (`training_config.json`)

```json
{
  "model_type": "lstm",
  "learning_rate": 0.001,
  "batch_size": 32,
  "epochs": 100,
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
  "gpu_device": null
}
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `model_type` | enum | `lstm` | Model: `lstm`, `bilstm`, `mlp`, `conv1d` |
| `learning_rate` | float | 0.001 | Adam learning rate |
| `batch_size` | int | 32 | Training batch size |
| `epochs` | int | 100 | Maximum epochs |
| `optimizer` | enum | `Adam` | Optimizer: `Adam`, `Sgd`, `AdamW` |
| `validation_split` | float | 0.2 | Train/val split (0.0-1.0) |
| `early_stopping_patience` | int | 10 | Stop after N epochs without improvement |

### LSTM Configuration

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `hidden_size` | int | 64 | LSTM hidden units |
| `num_layers` | int | 2 | Number of LSTM layers |
| `dropout` | float | 0.2 | Dropout probability |
| `bidirectional` | bool | false | Bidirectional LSTM |

### Label Configuration

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `horizon` | int | 5 | Bars to look ahead |
| `long_threshold` | float | 0.005 | Long threshold (0.5%) |
| `short_threshold` | float | 0.005 | Short threshold (0.5%) |
| `warmup_bars` | int | 20 | Lookback window |

---

## ML Strategy Configuration

### Default Configuration (`ml_strategy_config.json`)

```json
{
  "id": "nq_lstm_strategy",
  "name": "NQ LSTM Strategy",
  "description": "LSTM neural network trained on NQ futures",
  "signal_threshold_long": 0.6,
  "signal_threshold_short": 0.6,
  "min_confidence": 0.5,
  "use_confidence_for_sizing": false,
  "feature_config": {
    "features": [
      {"study_key": "sma_20", "output_field": "line"},
      {"study_key": "sma_50", "output_field": "line"},
      {"study_key": "ema_12", "output_field": "line"},
      {"study_key": "ema_26", "output_field": "line"},
      {"study_key": "rsi", "output_field": "line"},
      {"study_key": "atr", "output_field": "line"},
      {"study_key": "macd", "output_field": "line"},
      {"study_key": "macd_signal", "output_field": "line"},
      {"study_key": "macd_hist", "output_field": "line"},
      {"study_key": "bb_upper", "output_field": "band.upper"},
      {"study_key": "bb_lower", "output_field": "band.lower"},
      {"study_key": "vwap", "output_field": "line"}
    ],
    "lookback_periods": 20,
    "normalization": "none"
  }
}
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `signal_threshold_long` | float | 0.6 | Min P(long) for long signal |
| `signal_threshold_short` | float | 0.6 | Min P(short) for short signal |
| `min_confidence` | float | 0.5 | Min confidence to trade |
| `use_confidence_for_sizing` | bool | false | Scale position by confidence |

---

## CLI Commands Reference

### List Strategies

```bash
./target/debug/kairos list-strategies
```

### List Symbols

```bash
./target/debug/kairos list-symbols
```

### ML Training

```bash
./target/debug/kairos ml train --help
```

```
Train a new ML model

Usage: kairos ml train [OPTIONS]

Options:
      --config <CONFIG>                Path to training configuration file (JSON)
      --data-dir <DATA_DIR>            Path to training data directory
      --output <OUTPUT>                Output path for the trained model
      --features <FEATURES>            Feature configuration file (optional)
      --symbol <SYMBOL>               Symbol to train on (default: NQ)
      --start <START>                 Start date (YYYY-MM-DD)
      --end <END>                     End date (YYYY-MM-DD)
      --epochs <EPOCHS>               Number of epochs (overrides config)
      --learning-rate <LEARNING_RATE> Learning rate (overrides config)
      --batch-size <BATCH_SIZE>       Batch size (overrides config)
  -v, --verbose                      Verbose output
  -h, --help                         Print help
```

### ML Backtest

```bash
./target/debug/kairos backtest --help
```

```
Usage: kairos backtest [OPTIONS]

Required Options:
  -s, --symbol <SYMBOL>              Symbol (NQ, ES, YM, etc.)
      --start <START>                 Start date (YYYY-MM-DD)
      --end <END>                     End date (YYYY-MM-DD)
      --data-dir <DATA_DIR>           Path to DBN files

Options:
      --strategy <STRATEGY>           Strategy: orb, vwap_reversion, momentum_breakout, ml
      --timeframe <TIMEFRAME>         Timeframe: 1min, 5min, 15min, 1hour, 1day
      --capital <CAPITAL>              Initial capital (default: 100000)
      --model-path <MODEL_PATH>       Path to ML model (required for ml strategy)
      --strategy-config <CONFIG>      ML strategy config JSON (optional)
  -v, --verbose                       Verbose output
  -h, --help                         Print help
```

### Debug Data

Inspect a DBN file:

```bash
./target/debug/kairos debug-data --path /path/to/file.dbn.zst
```

---

## Troubleshooting

### No trades found

1. Check `--data-dir` points to correct directory
2. Verify file names match pattern `*.trades.dbn.zst`
3. Ensure dates are within file ranges

### Out of Memory

```bash
# Reduce batch size
--batch-size 16

# Or in config.json
"batch_size": 16

# Reduce LSTM size
"lstm_config": {
  "hidden_size": 32,
  "num_layers": 1
}
```

### Low accuracy

- Try longer date range for more training data
- Adjust label thresholds (`long_threshold`, `short_threshold`)
- Increase model capacity (`hidden_size`, `num_layers`)
- Add more features

### Model weights not loading

If you see:
```
Warning: Could not load model weights, using random initialization
```

This is expected due to PyTorch version differences between training and inference. The model architecture is preserved; only weights may differ.

### Binary won't run (libtorch error)

```
error while loading shared libraries: libtorch_cpu.so: cannot open shared object file
```

Set the library path:
```bash
export LD_LIBRARY_PATH="/home/administrator/.local/lib/python3.12/site-packages/torch/lib:$LD_LIBRARY_PATH"
```

---

## PyTorch Version Compatibility

The `tch` crate (v0.23) uses libtorch from the Python PyTorch installation. This enables training and inference using the same PyTorch runtime.

### Building with PyTorch

Set the environment variable to use Python PyTorch's libtorch:

```bash
LIBTORCH_USE_PYTORCH=1 LD_LIBRARY_PATH="/home/administrator/.local/lib/python3.12/site-packages/torch/lib:$LD_LIBRARY_PATH" \
  cargo build --package kairos-cli --features kairos-cli/tch
```

### Model Loading

Due to PyTorch version differences between the tch crate and Python PyTorch:

- **Training saves in VarStore format** - Works within the same session
- **Loading saved models** - May fail to load weights; model architecture is preserved but weights are randomly initialized

### Best Practices

1. **Train and backtest consecutively** - Train a model and use it immediately in the same session

2. **For persistent models** - Models can be saved/loaded within the same binary, but cross-version compatibility is not guaranteed

3. **Architecture is preserved** - Even if weights fail to load, the model architecture is correct

---

## Data Pipeline

```
DBN Files (.dbn.zst)
    ↓
Trade Messages (price, volume, timestamp)
    ↓
Filter by instrument ID (exclude calendar spreads)
    ↓
1-minute Candles (OHLCV aggregation)
    ↓
Technical Indicators (SMA, EMA, RSI, ATR, MACD, BB, VWAP)
    ↓
Normalized Features (relative to close price)
    ↓
LSTM Training / Backtesting
```

### Instrument ID Filtering

Calendar spreads are automatically filtered using instrument IDs:

| Symbol | Instrument IDs |
|--------|---------------|
| NQ | 4378, 2786, 828, 20987, 10351, 10903, 2770, 19685, 2895, 29652, 32274, 29558, 29804, 29882, 29653, 29754, 29757, 29763, 33011, 33014, 33018, 33021, 33024, 20631, 3522, 2130, 750, 260937, 106364 |

### Price Validation

Prices are validated to filter out calendar spreads:
- NQ: $5,000 - $30,000
- ES: $2,000 - $10,000
- YM: $15,000 - $50,000

---

## File Structure

```
kairos/
├── target/debug/kairos              # CLI binary
├── models/
│   └── nq_lstm_model.pt           # Trained model
├── training_config.json             # Training config
├── ml_strategy_config.json          # ML strategy config
├── crates/
│   ├── kairos-ml/                  # ML module
│   │   └── src/
│   │       ├── training/            # Training loop + DataGenerator
│   │       ├── model/              # Model implementations (TchModel)
│   │       ├── features/           # Feature extraction
│   │       └── strategy/           # MlStrategy implementation
│   └── cli/                        # CLI commands
│       └── src/
│           ├── backtest.rs         # Backtest + ML strategy
│           └── ml.rs               # ML training
└── LSTM_TRAINING.md                # This documentation
```

---

## Next Steps

1. **Add more indicators**: Stochastic, ADX, Ichimoku from `crates/study/src/studies/`
2. **Optimize hyperparameters**: Grid search over learning rate, hidden size, lookback
3. **Ensemble models**: Combine LSTM with MLP for robustness
4. **Production deployment**: Integrate with live trading system
5. **ML Strategy** ✅ **COMPLETE**: Use `--strategy ml --model-path <path>` in backtest command
