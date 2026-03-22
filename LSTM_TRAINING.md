# LSTM Neural Network Trading Strategy - Kairos

## Quick Start Guide

This guide will help you train and backtest an LSTM neural network trading strategy using **real market data** from DBN files, entirely in Rust.

### Prerequisites

1. **Rust** (1.94.0+)
2. **NQ Futures DBN Data Files** (in `../nq`)

### Build Kairos

```bash
cd /data/jbutler/algo-data/kairos

# Build the CLI with ML support
cargo build --package kairos-cli --features kairos-cli/tch
```

### Train an LSTM Model

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

Expected output:
```
ML Training
==========
Config: training_config.json
Data:   ../nq
Output: models/nq_lstm_model.pt

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
Epoch   1: train_loss=0.0249, val_loss=+1.1486, train_acc=Some(0.9981)
...
Training complete!
  Epochs trained: 50
  Final train loss: 0.0276
  Final val loss:   1.1403
Model saved to: models/nq_lstm_model.pt
```

### Run Backtest

```bash
./target/debug/kairos backtest \
  --symbol NQ \
  --start 2021-03-10 \
  --end 2021-03-31 \
  --strategy orb \
  --data-dir ../nq \
  --capital 100000 \
  --timeframe 1min \
  --verbose
```

## Features from Real Market Data

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

**Total: 12 features per timestep, 20 lookback timesteps = 240 input features**

**Important**: All features are normalized relative to the closing price. No raw price values are passed to the model.

## Architecture

### LSTM Model

```
Input Layer:  [batch, lookback=20, features=12]
    ↓
LSTM Layer:  64 hidden units, 2 layers
    ↓
Dense Layer:  hidden → 3 (long/neutral/short)
    ↓
Output:      Class probabilities
```

### Features

The model uses technical indicators computed from **real OHLCV data**:
- **Trend**: SMA (20, 50), EMA (12, 26)
- **Momentum**: RSI (14), MACD
- **Volatility**: ATR (14), Bollinger Bands
- **Volume**: VWAP

## Label Generation

Labels are generated based on **future returns**:

- **Long (0)**: Future return > long_threshold (default 0.5%)
- **Neutral (1)**: Return between thresholds
- **Short (2)**: Future return < short_threshold (default -0.5%)

## Configuration Reference

### Training Configuration (`training_config.json`)

```json
{
  "model_type": "LSTM",
  "learning_rate": 0.001,
  "batch_size": 32,
  "epochs": 100,
  "optimizer": "Adam",
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
  }
}
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `model_type` | enum | `LSTM` | Model architecture (LSTM, BiLSTM, MLP) |
| `learning_rate` | float | 0.001 | Adam learning rate |
| `batch_size` | int | 32 | Training batch size |
| `epochs` | int | 100 | Maximum epochs |
| `optimizer` | enum | `Adam` | Optimizer (Adam, Sgd, AdamW) |
| `validation_split` | float | 0.2 | Train/val split ratio |
| `early_stopping_patience` | int | 10 | Early stop epochs |

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
| `horizon` | int | 5 | Bars to look ahead for label |
| `long_threshold` | float | 0.005 | Return threshold for Long (0.5%) |
| `short_threshold` | float | 0.005 | Return threshold for Short (-0.5%) |
| `warmup_bars` | int | 20 | Lookback window for features |

## Data Loading (Rust Only)

### DBN File Format

The CLI reads Databento DBN files:

```
glbx-mdp3-20210310-20210331.trades.dbn.zst  ← NQ trades, March 2021
glbx-mdp3-20210401-20210430.trades.dbn.zst  ← NQ trades, April 2021
```

### Data Pipeline

```
DBN Files (.dbn.zst)
    ↓
Trade Messages (price, volume, timestamp)
    ↓
1-minute Candles (OHLCV aggregation)
    ↓
Technical Indicators (SMA, EMA, RSI, ATR, MACD, BB, VWAP)
    ↓
Normalized Features (relative to close price)
    ↓
LSTM Training
```

### Instrument ID Filtering

Calendar spreads are automatically filtered out using instrument IDs:

| Symbol | Instrument IDs |
|--------|---------------|
| NQ | 4378, 2786, 828, 20987, 10351, 10903, 2770, 19685, 2895, 29652, 32274, 29558, 29804, 29882, 29653, 29754, 29757, 29763, 33011, 33014, 33018, 33021, 33024, 20631, 3522, 2130, 750, 260937, 106364 |

### Price Validation

Prices are validated to filter out calendar spreads:
- NQ: $5,000 - $30,000
- ES: $2,000 - $10,000
- YM: $15,000 - $50,000

## File Structure

```
kairos/
├── target/debug/kairos              # CLI binary
├── models/
│   └── nq_lstm_model.pt           # Trained model
├── training_config.json              # Training config
├── crates/
│   ├── kairos-ml/                # ML module
│   │   └── src/
│   │       ├── training/           # Training loop + DataGenerator
│   │       ├── model/              # Model implementations
│   │       └── features/           # Feature extraction
│   └── cli/                        # CLI commands
│       └── src/
│           ├── backtest.rs         # Backtest command
│           └── ml.rs               # ML training (real data loading)
└── docs/
    └── LSTM_TRAINING.md            # This documentation
```

## Troubleshooting

### No trades found

The data files must contain the requested date range. Check:
1. `--data-dir` points to correct directory
2. File names match pattern `*.trades.dbn.zst`
3. Dates are within file ranges

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
- Adjust label thresholds (long_threshold, short_threshold)
- Increase model capacity (hidden_size, num_layers)
- Add more features

## Next Steps

1. **Add more indicators**: Add Stochastic, ADX, Ichimoku from `crates/study/src/studies/`
2. **Optimize hyperparameters**: Grid search over learning rate, hidden size, lookback
3. **Ensemble models**: Combine LSTM with MLP for robustness
4. **Production deployment**: Integrate with live trading system
5. **ML Strategy**: Create a strategy that uses the trained model for trading decisions
