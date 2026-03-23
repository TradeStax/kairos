# Sliding Window LSTM Training & Evaluation

This directory contains the scripts for running sliding window training and evaluation of LSTM models on NQ futures data, with comprehensive visualizations.

## Features

- **Sliding Window Training**: Train on 3 months, test on 1 month
- **Automated Backtesting**: Full backtest engine with realistic fill simulation
- **Visual Reports**: Interactive HTML reports with embedded charts
- **Performance Metrics**: Sharpe, Sortino, Calmar ratios, drawdown analysis

## Charts Generated

| Chart | Description |
|-------|-------------|
| Equity Curve | Account equity over time with realized/unrealized P&L |
| Drawdown | Peak-to-trough drawdown visualization |
| Trade Distribution | Histogram of individual trade P&Ls |
| Monthly Returns | Bar chart of returns across windows |
| Daily P&L | Distribution and cumulative daily P&L |
| Metrics Radar | Normalized performance metrics comparison |

## Usage

### Quick Start

Run the complete pipeline (builds Kairos + trains + backtests):

```bash
cd /data/jbutler/algo-data/kairos
./scripts/run_sliding_window.sh
```

### Command Line Options

```bash
# Run with custom parameters
python3 scripts/sliding_window_train_eval.py \
    --train-months 3 \
    --test-months 1 \
    --num-windows 3 \
    --start-date 2021-03-01 \
    --epochs 50 \
    --capital 100000

# Skip training (use existing models)
python3 scripts/sliding_window_train_eval.py --skip-training

# Skip chart generation
python3 scripts/sliding_window_train_eval.py --skip-charts

# Verbose output
python3 scripts/sliding_window_train_eval.py --verbose
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--train-months` | 3 | Number of months for training |
| `--test-months` | 1 | Number of months for testing |
| `--num-windows` | 3 | Number of sliding windows |
| `--start-date` | 2021-03-01 | Start date for first window |
| `--timeframe` | 5min | Candle timeframe |
| `--symbol` | NQ | Futures symbol |
| `--epochs` | 50 | Training epochs |
| `--capital` | 100000 | Initial backtest capital |
| `--data-dir` | /data/jbutler/algo-data/nq | Data directory |
| `--skip-training` | False | Skip training phase |
| `--skip-backtest` | False | Skip backtesting phase |
| `--skip-charts` | False | Skip chart generation |
| `--verbose` | False | Verbose output |

## Output

### Reports Location

All reports are saved to `/data/jbutler/algo-data/kairos/reports/`:

```
reports/
├── cumulative_report.html     # Interactive HTML with all charts
├── cumulative_report.json    # Full cumulative data
├── cumulative_report.txt     # Human-readable summary
└── monthly/
    ├── window_01_export.json      # Raw Kairos export
    ├── window_01_2021-06.html    # Interactive window report
    ├── window_01_2021-06.json    # Window metrics
    ├── window_02_export.json
    ├── window_02_2021-10.html
    ├── window_02_2021-10.json
    └── ...
```

### HTML Reports

The HTML reports include:

#### Monthly Report (Window-level)
- Performance summary cards with key metrics
- Trade statistics table
- Risk metrics (Sharpe, Sortino, Calmar, Expectancy)
- Equity curve chart
- Drawdown chart
- Trade P&L distribution
- Performance metrics radar chart
- Training information

#### Cumulative Report (Portfolio-level)
- Overall performance summary
- Per-window comparison table
- Training summary table
- Monthly returns bar chart
- Daily P&L analysis
- Cumulative equity curve

### JSON Report Structure

```json
{
  "report_type": "monthly_window_performance",
  "window": 1,
  "train_period": { "start": "2021-03-01", "end": "2021-05-31" },
  "test_period": { "start": "2021-06-01", "end": "2021-06-30" },
  "training": {
    "epochs_trained": 50,
    "final_train_loss": 0.8234,
    "final_val_loss": 0.9156,
    "num_samples": 12500
  },
  "backtest": {
    "net_pnl_usd": 2500.00,
    "total_return_pct": 2.5,
    "max_drawdown_pct": 3.2,
    "total_trades": 45,
    "win_rate": 0.62,
    "profit_factor": 1.85,
    "sharpe_ratio": 1.23,
    "sortino_ratio": 1.56,
    "html_report": "monthly/window_01_2021-06.html"
  }
}
```

## Sliding Window Process

The sliding window approach:

```
Window 1:
  Train: 2021-03-01 to 2021-05-31 (3 months)
  Test:  2021-06-01 to 2021-06-30 (1 month)
  Model: nq_lstm_window01

Window 2:
  Train: 2021-07-01 to 2021-09-30 (3 months)
  Test:  2021-10-01 to 2021-10-31 (1 month)
  Model: nq_lstm_window02

Window 3:
  Train: 2021-11-01 to 2022-01-31 (3 months)
  Test:  2022-02-01 to 2022-02-28 (1 month)
  Model: nq_lstm_window03
```

## Models

Trained models are saved to `/data/jbutler/algo-data/kairos/models/`:

```
models/
├── nq_lstm_window01.safetensors  # Model weights
├── nq_lstm_window01.json         # Model metadata
├── nq_lstm_window01_strategy.json # Strategy config
├── nq_lstm_window02.safetensors
├── ...
```

## Individual Commands

### Train a single model

```bash
./target/debug/kairos ml train \
    --config models/training_config.json \
    --data-dir /data/jbutler/algo-data/nq \
    --output models/my_model \
    --symbol NQ \
    --start 2021-03-01 \
    --end 2021-05-31 \
    --timeframe 5min \
    --epochs 50
```

### Backtest a single model

```bash
./target/debug/kairos backtest \
    --symbol NQ \
    --start 2021-06-01 \
    --end 2021-06-30 \
    --strategy ml \
    --model-path models/my_model.safetensors \
    --timeframe 5min \
    --data-dir /data/jbutler/algo-data/nq \
    --capital 100000 \
    --format json \
    --export results.json
```

## Viewing Reports

### View in Browser

```bash
# Open the cumulative report
firefox reports/cumulative_report.html

# Open a specific window report
firefox reports/monthly/window_01_2021-06.html
```

### View JSON Data

```bash
# Pretty print the cumulative report
cat reports/cumulative_report.json | python3 -m json.tool

# Extract specific metrics
cat reports/cumulative_report.json | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'Cumulative Return: {d[\"cumulative_metrics\"][\"cumulative_return_pct\"]:.2f}%')"
```

## Troubleshooting

### "No trades found"

1. Check that `--data-dir` points to the correct directory
2. Verify file names match the expected pattern (`*.dbn.zst`)
3. Check date ranges are valid

### Charts not generated

1. Ensure matplotlib is installed:
   ```bash
   pip install matplotlib
   ```
2. Check for matplotlib errors in output
3. Use `--skip-charts` to disable chart generation

### Training fails

1. Ensure GPU environment is set up:
   ```bash
   export LIBTORCH_USE_PYTORCH=1
   export LD_LIBRARY_PATH="/home/administrator/.local/lib/python3.12/site-packages/torch/lib:/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
   ```

### Backtest fails

1. Verify model file exists
2. Check training timeframe matches backtest timeframe (5min)
3. Ensure strategy config is valid JSON
