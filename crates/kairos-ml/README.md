# Kairos ML Module

ML strategy module for Kairos with PyTorch integration for building and deploying machine learning trading strategies.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Tutorial](#tutorial)
- [Usage Examples](#usage-examples)
- [Architecture](#architecture)
- [CLI Commands](#cli-commands)
- [Performance](#performance)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)

---

## Features

- **Model Loading**: Load models from safetensors format with architecture metadata
- **Feature Extraction**: Convert any combination of studies to model input tensors
- **Inference Engine**: Batch prediction with signal output and confidence scores
- **Training Pipeline**: Train models using built-in indicators as features
- **ML Strategy**: `MlStrategy` implementing the `Strategy` trait for backtesting

---

## Installation

### Prerequisites

- **Rust 1.75+** (2024 edition)
- **libtorch** (automatically downloaded by `tch` crate, or install manually)
- **C compiler** (gcc/clang) for linking

### Install libtorch Manually (Optional)

For faster builds or custom libtorch versions:

```bash
# Download libtorch CPU version
wget https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-2.3.0%2Bcpu.zip
unzip libtorch-cxx11-abi-shared-with-deps-2.3.0+cpu.zip

# Set environment variable
export LIBTORCH_DIR=/path/to/libtorch
export LD_LIBRARY_PATH=$LIBTORCH_DIR/lib:$LD_LIBRARY_PATH
```

### Build

```bash
# Build the crate
cargo build -p kairos-ml

# Build with optimizations
cargo build -p kairos-ml --release

# Build with ONNX support (optional)
cargo build -p kairos-ml --features ort
```

---

## Quick Start

### 1. Train a Model

```bash
cargo run --example train_simple_model --release
```

This will:
- Generate synthetic training data
- Train a simple MLP classifier
- Save the model to `trained_model.safetensors`

### 2. Validate the Model

```bash
kairos ml validate-model --model trained_model.safetensors --data sample.dbn
```

### 3. Run a Backtest

```bash
kairos backtest \
  --symbol NQ \
  --start 2023-01-01 \
  --end 2023-12-31 \
  --strategy ml_strategy \
  --data-dir /path/to/dbn/files \
  --model trained_model.safetensors
```

---

## Tutorial

### Step 1: Define Your Feature Set

Features are derived from study outputs (indicators). Each feature maps to:
- A study key (e.g., "sma_20", "rsi_14")
- An output field (e.g., "line", "value", "band.upper")

```rust
use kairos_ml::features::{FeatureConfig, FeatureDefinition, NormalizationMethod};

let feature_config = FeatureConfig {
    features: vec![
        // Simple Moving Average
        FeatureDefinition::new("sma_20", "line"),
        // Relative Strength Index
        FeatureDefinition::new("rsi_14", "value"),
        // MACD with percentage change transform
        FeatureDefinition::new("macd", "value")
            .with_transform(FeatureTransform::PctChange),
    ],
    lookback_periods: 20,
    normalization: NormalizationMethod::ZScore,
};
```

### Step 2: Configure the ML Strategy

Set up signal thresholds and model path:

```rust
use kairos_ml::{MlStrategy, MlStrategyConfig};

let ml_config = MlStrategyConfig::new(feature_config)
    .model_path("trained_model.safetensors")
    .signal_threshold_long(0.6)    // 60% probability for long
    .signal_threshold_short(0.6)  // 60% probability for short
    .min_confidence(0.5);          // 50% confidence minimum

let strategy = MlStrategy::new(ml_config);
```

### Step 3: Run the Backtest

The strategy integrates with the Kairos backtest engine:

```rust
use kairos_backtest::engine::BacktestEngine;

let mut engine = BacktestEngine::new(config);
engine.add_strategy(strategy);
let result = engine.run().await?;
```

---

## Usage Examples

### Feature Extraction

```rust
use kairos_ml::features::{FeatureConfig, FeatureDefinition};
use kairos_ml::features::StudyFeatureExtractor;

let config = FeatureConfig {
    features: vec![
        FeatureDefinition::new("sma_20", "line"),
        FeatureDefinition::new("rsi_14", "value"),
    ],
    lookback_periods: 20,
    normalization: NormalizationMethod::ZScore,
};

let mut extractor = StudyFeatureExtractor::new(config);
extractor.add_scalar("sma_20", 1500.0, 1000);
extractor.add_scalar("rsi_14", 65.0, 1000);

let features = extractor.extract(20)?;
```

### Model Inference

```rust
use kairos_ml::model::{Model, TchModel, ModelOutput};

let model = TchModel::new(10, 64, 3, "classifier");
let input = tch::Tensor::randn([1, 1, 10], (tch::Kind::Float, tch::Device::Cpu));

let output = model.predict(&input)?;
match output {
    ModelOutput::Classification { probabilities, prediction } => {
        println!("Signal: {:?}, Confidence: {:.2}", prediction, probabilities);
    }
    ModelOutput::Regression { value } => {
        println!("Predicted value: {:.4}", value);
    }
}
```

### Training Pipeline

```rust
use kairos_ml::training::{TrainingConfig, Dataset};
use kairos_ml::training::training_loop::train;

let config = TrainingConfig::default();
let dataset = create_dataset_from_candles(candles, &feature_config, &label_config);

let result = train(&config, &dataset, &LoggingCallback)?;
println!("Trained for {} epochs", result.epochs_trained);
```

### Signal Generation

```rust
use kairos_ml::model::{TradingSignal, ModelOutput};

let probabilities = [0.7, 0.2, 0.1]; // 70% long, 20% neutral, 10% short

match output {
    ModelOutput::Classification { probabilities, prediction } => {
        match prediction {
            TradingSignal::Long => println!("Generate long order"),
            TradingSignal::Short => println!("Generate short order"),
            TradingSignal::Neutral => println!("No order"),
        }
    }
}
```

---

## Architecture

```
kairos-ml/
├── src/
│   ├── lib.rs              # Main module and re-exports
│   ├── model/              # Model loading and inference
│   │   ├── mod.rs          # Model trait definition
│   │   ├── output.rs      # ModelOutput, TradingSignal types
│   │   ├── registry.rs    # Model registry
│   │   └── tch_impl.rs    # TchModel implementation
│   ├── features/           # Feature extraction pipeline
│   │   ├── mod.rs         # FeatureExtractor trait
│   │   ├── config.rs      # FeatureConfig types
│   │   └── extractor.rs   # StudyFeatureExtractor impl
│   ├── training/           # Training pipeline
│   │   ├── mod.rs
│   │   ├── config.rs      # TrainingConfig types
│   │   ├── dataset.rs     # Dataset structures
│   │   ├── data_generator.rs
│   │   └── training_loop.rs
│   └── strategy/           # ML Strategy wrapper
│       ├── mod.rs         # MlStrategy implementation
│       └── config.rs      # MlStrategyConfig
├── examples/               # Usage examples
│   ├── train_simple_model
│   └── ml_strategy_backtest.rs
├── benches/                # Performance benchmarks
└── tests/                 # Integration tests
```

### Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        Backtest Engine                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐  │
│  │   Candles    │───▶│    Study     │───▶│  Feature Matrix   │  │
│  │              │    │    Bank      │    │   (Tensor)       │  │
│  └──────────────┘    └──────────────┘    └────────┬─────────┘  │
│                                                    │            │
│                                                    ▼            │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐  │
│  │   Order      │◀───│   Signal     │◀───│   Model          │  │
│  │   Requests   │    │  Generator   │    │   Inference      │  │
│  └──────────────┘    └──────────────┘    └──────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## CLI Commands

### Train a Model

```bash
kairos ml train \
  --config training_config.json \
  --data-dir /path/to/dbn/files \
  --output trained_model.safetensors \
  --epochs 100 \
  --learning-rate 0.001
```

### List Available Models

```bash
kairos ml list-models
```

### Validate a Model

```bash
kairos ml validate-model \
  --model trained_model.safetensors \
  --data sample.dbn \
  --num-samples 1000 \
  --verbose
```

---

## Performance

### Targets

| Metric | Target | Description |
|--------|--------|-------------|
| Inference latency | < 10ms | Single prediction on CPU |
| Feature extraction | < 5ms | 100 studies per candle |
| Training throughput | > 1000 samples/s | Training speed on GPU |

### Run Benchmarks

```bash
cargo bench -p kairos-ml
```

### Model Persistence

Models are saved using the **safetensors** format:

```
models/
├── nq_lstm_model.safetensors  # Model weights
└── nq_lstm_model.json         # Architecture metadata
```

The metadata JSON contains:
- `model_type`: "lstm" or "mlp"
- `num_features`: Number of input features
- `lookback`: Sequence length
- `hidden_size`: Hidden layer size
- `num_classes`: Output classes (3 for long/neutral/short)

---

## Troubleshooting

### Build Errors

#### "Cannot find libtorch"

```bash
# Option 1: Let tch download automatically (requires internet)
cargo build -p kairos-ml

# Option 2: Set LIBTORCH_DIR manually
export LIBTORCH_DIR=/path/to/libtorch
cargo build -p kairos-ml
```

#### "C compiler not found"

Install a C compiler:

```bash
# Ubuntu/Debian
sudo apt install build-essential

# macOS
xcode-select --install

# Windows
# Install Visual Studio Build Tools
```

### Runtime Errors

| Error | Cause | Solution |
|-------|-------|----------|
| "Model not found" | Invalid model path | Check file exists and path is correct |
| "Insufficient data" | Not enough bars | Increase lookback period or use more data |
| "Invalid input shape" | Model/feature mismatch | Ensure feature config matches model input |

### Performance Issues

| Symptom | Possible Cause | Solution |
|---------|----------------|----------|
| Slow inference | Large model | Use smaller architecture or batch processing |
| Memory errors | Large lookback | Reduce lookback period |
| Slow training | CPU only | Use GPU-enabled libtorch |

---

## Contributing

Contributions are welcome! Please see the main Kairos contributing guide.

### Development Workflow

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Run tests: `cargo test -p kairos-ml`
4. Run clippy: `cargo clippy -p kairos-ml -- -D warnings`
5. Commit your changes
6. Push and create a Pull Request

### Code Style

- Follow Rust idioms and conventions
- Use `cargo fmt` for formatting
- Add doc comments to public APIs
- Include tests for new functionality

---

## License

Same as Kairos project.
