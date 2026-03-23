# LSTM Neural Network Trading Strategy - Kairos

A complete guide to training and backtesting LSTM neural network trading strategies using real market data, entirely in Rust with GPU acceleration.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [GPU Setup](#gpu-setup)
3. [Building](#building)
4. [Training Models](#training-models)
5. [ML Strategy Backtesting](#ml-strategy-backtesting)
6. [Stop-Loss/Take-Profit](#stop-losstake-profit)
7. [Performance Tuning](#performance-tuning)
8. [Configuration Reference](#configuration-reference)
9. [Troubleshooting](#troubleshooting)

---

## Prerequisites

1. **Rust** (1.94.0+)
2. **NQ Futures DBN Data Files** (in `/data/jbutler/algo-data/nq` or configured data directory)
3. **Python PyTorch** (for runtime library access)
4. **NVIDIA GPU** (optional but recommended for training)

## GPU Setup

### Verify GPU Available

```bash
nvidia-smi
```

### Environment Variables

```bash
export LIBTORCH_USE_PYTORCH=1
export LD_LIBRARY_PATH="/home/administrator/.local/lib/python3.12/site-packages/torch/lib:/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
```

Add to your shell profile:
```bash
echo 'export LIBTORCH_USE_PYTORCH=1' >> ~/.bashrc
echo 'export LD_LIBRARY_PATH="/home/administrator/.local/lib/python3.12/site-packages/torch/lib:/usr/local/cuda/lib64:$LD_LIBRARY_PATH"' >> ~/.bashrc
source ~/.bashrc
```

## Building

```bash
cargo build --package kairos-cli --features kairos-cli/tch
```

---

## Training Models

### Quick Start Training (GPU)

```bash
./target/debug/kairos ml train \
  --config training_config.json \
  --data-dir /data/jbutler/algo-data/nq \
  --output models/nq_lstm_model.safetensors \
  --epochs 50 \
  --start 2021-01-01 \
  --end 2021-06-30 \
  --timeframe 5min \
  --verbose
```

### Timeframe Configuration

The `--timeframe` option controls candle aggregation for both training and backtesting:

| Flag | Default | Description |
|------|---------|-------------|
| `--timeframe` | `1min` | Candle timeframe for feature aggregation |

**Supported timeframes:**
- `1s`, `5s`, `10s`, `30s` - Second-based (high-frequency)
- `1min`, `3min`, `5min`, `15min`, `30min` - Minute-based
- `1hour`, `4hour` - Hour-based
- `1day` - Daily bars

**Example - Training on 5-minute bars:**
```bash
./target/debug/kairos ml train \
  --config training_config.json \
  --data-dir /data/jbutler/algo-data/nq \
  --output models/nq_lstm_5min.safetensors \
  --timeframe 5min \
  --epochs 50
```

**Important:** When backtesting with the trained model, use the **same timeframe** to ensure feature alignment:
```bash
./target/debug/kairos backtest \
  --symbol NQ \
  --start 2021-03-15 \
  --end 2021-03-20 \
  --strategy ml \
  --model-path models/nq_lstm_5min.safetensors \
  --timeframe 5min \
  --data-dir /data/jbutler/algo-data/nq
```

### Timeframe Considerations

| Timeframe | Bars/Day (9:30-16:00) | Best For |
|-----------|----------------------|----------|
| `1min` | ~390 | Scalping, high-frequency signals |
| `5min` | ~78 | Short-term swings, day trading |
| `15min` | ~26 | Swing trading, position building |
| `1hour` | ~7 | Position trading, overnight holds |

**Note:** Candle aggregation aligns to 0-minute boundaries (e.g., 09:00, 09:05, 09:10 for 5min bars).

### Training Output

```
Training Configuration:
  Model type:        LSTM
  Learning rate:     0.001
  Batch size:       256
  Epochs:           50
  Timeframe:        5min
  Validation split:  0.2
  Early stopping:    10 epochs

Dataset created:
  Total samples: 218,853
  Lookback:      20
  Features:      12

Starting training...
Using GPU 0
Device: Cuda(0)
Training: features=12, lookback=20, classes=3, architecture=LSTM
Epoch   1: train_loss=0.0999, val_loss=-0.0919, train_acc=Some(0.988)
...
Early stopping at epoch 12

Model saved successfully!
  Input shape:    [batch, 240] (lookback=20, features=12)
  Output shape:   [batch, 3] (long, neutral, short)
```

---

## ML Strategy Backtesting

### Basic ML Backtest

```bash
./target/debug/kairos backtest \
  --symbol NQ \
  --start 2021-03-15 \
  --end 2021-03-20 \
  --strategy ml \
  --model-path models/nq_lstm_model.safetensors \
  --strategy-config ml_strategy_config.json \
  --timeframe 5min \
  --data-dir /data/jbutler/algo-data/nq \
  --capital 100000 \
  --verbose
```

### Backtest Output

```
ML Strategy Configuration
========================
Model: models/nq_lstm_model.safetensors
Features: 12 indicators
  - SMA (20, 50), EMA (12, 26), RSI (14), ATR (14)
  - MACD (12, 26, 9), Bollinger Bands (20, 2), VWAP
Lookback: 20 bars

Results
=======
Final Equity: $129,270.00
Return: 29.27%
Max Drawdown: 3.35% ($3,395.00)
Trades: 452
Win Rate: 63.1%
Profit Factor: 3.42
Sharpe: 19.06
Sortino: 134.60
```

---

## Stop-Loss/Take-Profit

The ML strategy supports bracket orders with configurable stop-loss and take-profit.

### Configuration

```json
{
  "sl_tp": {
    "stop_loss_ticks": 20,
    "take_profit_ticks": 30,
    "use_atr_based": false,
    "stop_loss_atr_multiplier": 2.0,
    "take_profit_atr_multiplier": 2.0
  }
}
```

### Options

| Parameter | Description |
|-----------|-------------|
| `stop_loss_ticks` | Stop-loss distance in ticks (NQ tick = $0.25) |
| `take_profit_ticks` | Take-profit distance in ticks |
| `use_atr_based` | Use ATR multipliers instead of fixed ticks |
| `*_atr_multiplier` | ATR multiplier when `use_atr_based` is true |

### Example Configurations

**Tight SL/TP (Higher Win Rate, Lower Reward)**:
```json
"sl_tp": {
  "stop_loss_ticks": 10,
  "take_profit_ticks": 15
}
```

**Wide SL/TP (Lower Win Rate, Higher Reward)**:
```json
"sl_tp": {
  "stop_loss_ticks": 40,
  "take_profit_ticks": 60
}
```

**ATR-Based (Adaptive to Volatility)**:
```json
"sl_tp": {
  "use_atr_based": true,
  "stop_loss_atr_multiplier": 2.0,
  "take_profit_atr_multiplier": 3.0
}
```

---

## Performance Tuning

### Signal Thresholds

| Threshold | Effect |
|-----------|--------|
| `signal_threshold_long` | Higher = fewer long signals |
| `signal_threshold_short` | Higher = fewer short signals |
| `min_confidence` | Filters low-confidence predictions |

### Recommended Starting Points

| Trading Style | Long/Short | Confidence | Trades/Day |
|---------------|------------|------------|------------|
| Conservative | 0.50 | 0.50 | 2-5 |
| Moderate | 0.45 | 0.40 | 5-15 |
| Aggressive | 0.35 | 0.30 | 15-50+ |

### Tested Configuration (Good In-Sample)

```json
{
  "signal_threshold_long": 0.45,
  "signal_threshold_short": 0.45,
  "min_confidence": 0.40,
  "sl_tp": {
    "stop_loss_ticks": 20,
    "take_profit_ticks": 30
  }
}
```

---

## Configuration Reference

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
  "use_confidence_for_sizing": false,
  "sl_tp": {
    "stop_loss_ticks": 20,
    "take_profit_ticks": 30,
    "use_atr_based": false,
    "stop_loss_atr_multiplier": 2.0,
    "take_profit_atr_multiplier": 2.0
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
    "lookback_periods": 20,
    "normalization": "none"
  }
}
```

---

## Features & Architecture

### 12 Technical Indicators

| Feature | Study ID | Output Field | Period |
|--------|----------|-------------|--------|
| `sma_20` | sma | line | 20 |
| `sma_50` | sma | line | 50 |
| `ema_12` | ema | line | 12 |
| `ema_26` | ema | line | 26 |
| `rsi` | rsi | value | 14 |
| `atr` | atr | value | 14 |
| `macd` | macd | lines.0 | 12,26,9 |
| `macd_signal` | macd | lines.1 | 12,26,9 |
| `macd_hist` | macd | histogram | 12,26,9 |
| `bollinger_upper` | bollinger | band.upper | 20,2 |
| `bollinger_lower` | bollinger | band.lower | 20,2 |
| `vwap` | vwap | value | - |

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

---

## Troubleshooting

### No trades generated

1. **Check confidence threshold** - Lower `min_confidence` in strategy config
2. **Check model predictions** - Run with `--verbose` to see signal probabilities
3. **Verify warmup** - Ensure `warm_up_periods` >= lookback (20)

### GPU not used for training

```bash
# Verify GPU is available
nvidia-smi

# Verify PyTorch CUDA
python3 -c "import torch; print(torch.cuda.is_available())"

# Check LD_LIBRARY_PATH
echo $LD_LIBRARY_PATH
```

### Model loading fails

Ensure both files exist:
```bash
ls models/nq_lstm_model.safetensors  # Model weights
ls models/nq_lstm_model.json         # Model metadata
```

### Low accuracy / no profitable trades

1. **Train on more data** - Use 1+ years of data
2. **Adjust thresholds** - Lower signal thresholds for more trades
3. **Optimize hyperparameters** - Try different learning rates, hidden sizes, dropout

---

## GPU Training Performance

| Dataset Size | Epochs | Time (GPU) | Time (CPU est.) |
|--------------|--------|------------|-----------------|
| 3 months | 20 | ~2 min | ~20 min |
| 6 months | 50 | ~5 min | ~1 hour |
| 1 year | 50 | ~10 min | ~2 hours |

**GPU**: NVIDIA RTX 3090, 24GB VRAM
**Batch Size**: 256 (optimal for GPU memory)
**Throughput**: ~50,000 samples/second on GPU

---

## Current Status

| Component | Status | Notes |
|-----------|--------|-------|
| GPU Training | ✅ Working | RTX 3090, batch_size=256 |
| Training Config | ✅ Working | JSON config with all parameters |
| Model Save/Load | ✅ Working | safetensors + JSON metadata |
| Feature Extraction | ✅ Working | All 12 indicators |
| ML Backtest | ✅ Working | Signals, orders, positions |
| Session Close | ✅ Working | Flattens positions properly |
| Stop-Loss/Take-Profit | ✅ Working | Bracket orders with configurable SL/TP |
| Profitability | ⚠️ Mixed | Needs more training data for generalization |

### Latest Backtest Results (In-Sample: March 15-20, 2021)

```
Final Equity: $129,270.00
Return: 29.27%
Max Drawdown: 3.35% ($3,395.00)
Trades: 452
Win Rate: 63.1%
Profit Factor: 3.42
Sharpe: 19.06
Sortino: 134.60
```

### Out-of-Sample Results (May 3-7, 2021)

```
Return: -25.34%
Max Drawdown: 25.34%
Win Rate: 21.7%
Profit Factor: 0.28
```

**Note**: Out-of-sample performance indicates need for more training data and better regularization.

---

## Next Steps

1. ~~**Add stop-loss/take-profit**~~ ✅ Done
2. ~~**Tune confidence thresholds**~~ ✅ Done
3. ~~**Different timeframes**~~ ✅ Done - Use `--timeframe` flag for training and backtesting
4. **Train on more data** - 1-3 years for better generalization
5. **Optimize hyperparameters** - Grid search learning rate, hidden size, dropout
6. **Ensemble models** - Combine multiple trained models
7. **Walk-forward analysis** - Test generalization across different time periods
