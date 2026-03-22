# LSTM Neural Network Trading Strategy - Kairos

## Quick Start Guide

This guide will help you train and backtest an LSTM neural network trading strategy using Kairos headless CLI.

### Prerequisites

1. **Rust** (1.94.0+)
2. **Python 3.12+ with PyTorch** (for GPU support)
3. **NQ Futures DBN Data Files** (in `../nq`)

### Build Kairos

```bash
cd /data/jbutler/algo-data/kairos

# Set up environment for PyTorch/libtorch
export LIBTORCH_USE_PYTORCH=1
export LD_LIBRARY_PATH=/home/administrator/.local/lib/python3.12/site-packages/torch/lib:$LD_LIBRARY_PATH

# Build the CLI
cargo build --package kairos-cli --features kairos-cli/tch
```

### Train an LSTM Model

1. **Create training configuration** (`training_config.json`):

```json
{
  "model_type": "LSTM",
  "learning_rate": 0.001,
  "batch_size": 32,
  "epochs": 100,
  "optimizer": "Adam",
  "validation_split": 0.2,
  "early_stopping_patience": 10,
  "lstm_config": {
    "hidden_size": 64,
    "num_layers": 2,
    "dropout": 0.2,
    "bidirectional": false
  }
}
```

2. **Run training**:

```bash
./target/debug/kairos ml train \
  --config training_config.json \
  --data-dir ../nq \
  --output models/nq_lstm_model.pt \
  --epochs 50 \
  --verbose
```

Expected output:
```
Device: CPU (GPU unavailable)
Training: features=3, lookback=20, classes=3, architecture=LSTM
Epoch   1: train_loss=1.1119, val_loss=-0.0037, train_acc=Some(0.3175)
Epoch   2: train_loss=1.1060, val_loss=+0.0006, train_acc=Some(0.33875)
...
Training complete!
  Epochs trained: 50
  Final train loss: 0.8234
  Final val loss:   0.8912
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

Expected output:
```
Kairos Backtest
Symbol: NQ | Period: 2021-03-10 to 2021-03-31 | Strategy: orb
Initial Capital: $100000.00
Loading trades...
Loaded 6429974 trades
Price range: $12602.00 to $13298.00 (avg: $12940.59)
Running backtest...

Results
=======
Final Equity: $89675.00
Return: -10.32%
Max Drawdown: 13.94% ($14300.00)
Trades: 9
Win Rate: 22.2%
```

## Architecture

### LSTM Model

```
Input Layer:  [batch, lookback, features]
    ↓
LSTM Layer:  64 hidden units, 2 layers
    ↓
Dense Layer:  hidden → 3 (long/neutral/short)
    ↓
Output:      Class probabilities
```

### Features

The model uses technical indicators:
- SMA (20, 50)
- EMA (12, 26)
- RSI (14)
- ATR (14)
- MACD
- Bollinger Bands
- VWAP

## Configuration Reference

### Training Configuration

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `model_type` | enum | `LSTM` | Model architecture |
| `learning_rate` | float | 0.001 | Adam learning rate |
| `batch_size` | int | 32 | Training batch size |
| `epochs` | int | 100 | Maximum epochs |
| `optimizer` | enum | `Adam` | Optimizer |
| `validation_split` | float | 0.2 | Train/val split ratio |
| `early_stopping_patience` | int | 10 | Early stop epochs |

### LSTM Configuration

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `hidden_size` | int | 64 | LSTM hidden units |
| `num_layers` | int | 2 | Number of LSTM layers |
| `dropout` | float | 0.2 | Dropout probability |
| `bidirectional` | bool | false | Bidirectional LSTM |

## Python Training Script

A more feature-rich Python training script is available at `scripts/train_lstm.py`:

```bash
python3 scripts/train_lstm.py \
  --data-dir ../nq \
  --start 2021-03-10 \
  --end 2021-03-31 \
  --epochs 50 \
  --output models/nq_lstm.pt
```

## Troubleshooting

### GPU Not Detected

```bash
# Check CUDA availability
python3 -c "import torch; print(f'CUDA: {torch.cuda.is_available()}')"

# Set GPU device
"gpu_device": 0
```

### Out of Memory

```bash
# Reduce batch size
"batch_size": 16

# Reduce LSTM size
"lstm_config": {
  "hidden_size": 32,
  "num_layers": 1
}
```

### No Trades Found

The data files must contain the requested date range. Available files:
```
glbx-mdp3-20210310-20210331.trades.dbn.zst  ← March 2021
glbx-mdp3-20210401-20210430.trades.dbn.zst  ← April 2021
...
```

## File Structure

```
kairos/
├── target/debug/kairos              # CLI binary
├── models/
│   └── nq_lstm_model.pt           # Trained model
├── training_config.json              # Training config
├── scripts/
│   └── train_lstm.py               # Python training script
├── crates/
│   ├── kairos-ml/                # ML module
│   │   └── src/
│   │       ├── training/           # Training loop
│   │       ├── model/              # Model implementations
│   │       └── features/           # Feature extraction
│   └── cli/                        # CLI commands
│       └── src/
│           ├── backtest.rs         # Backtest command
│           └── ml.rs               # ML training command
└── docs/
    └── LSTM_TRAINING.md            # This documentation
```

## Next Steps

1. **Add real indicators**: Replace synthetic features with actual study outputs
2. **Add data loading**: Implement DBN file reading for real market data
3. **Optimize hyperparameters**: Grid search over learning rate, hidden size, etc.
4. **Ensemble models**: Combine LSTM with MLP for robustness
5. **Production deployment**: Integrate with live trading system
