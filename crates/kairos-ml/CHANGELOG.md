# Changelog

All notable changes to the kairos-ml crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial implementation of kairos-ml crate

### Features

#### Phase 0: Project Setup
- [x] Create crate structure with Cargo.toml
- [x] Set up module organization (model/, features/, training/, strategy/)
- [x] Feature flags: `tch` (default), `ort` (optional)

#### Phase 1: Core Model Infrastructure
- [x] `Model` trait with `predict()`, `input_shape()`, `output_shape()` methods
- [x] `ModelOutput` enum with `Classification` and `Regression` variants
- [x] `TradingSignal` enum: `Long`, `Short`, `Neutral`
- [x] `TchModel` implementation using tch crate
- [x] `ModelRegistry` for centralized model loading

#### Phase 2: Feature Extraction Pipeline
- [x] `FeatureExtractor` trait
- [x] `FeatureConfig` and `FeatureDefinition` structs
- [x] `NormalizationMethod` enum: `ZScore`, `MinMax`, `None`
- [x] `FeatureTransform` enum: `Log`, `Diff`, `PctChange`
- [x] Support for LineSeries, Band, Bars study outputs
- [x] Rolling window buffer for lookback
- [x] Forward fill for missing values

#### Phase 3: ML Strategy Wrapper
- [x] `MlStrategyConfig` with builder pattern
- [x] `MlStrategy` implementing `Strategy` trait
- [x] Signal generation with configurable thresholds
- [x] Lifecycle methods: `on_init`, `on_candle`, `on_tick`, `reset`
- [x] Warm-up period handling
- [x] Model output to order request conversion

#### Phase 4: Training Pipeline
- [x] `TrainingConfig` with hyperparameters
- [x] `Dataset` struct with train/validation split
- [x] `LabelConfig` for label generation
- [x] `DataGenerator` for feature matrix generation
- [x] Training loop with mini-batch gradient descent
- [x] Cross-entropy loss computation
- [x] Early stopping with patience
- [x] `LoggingCallback` for progress tracking
- [x] Model export via VarStore

#### Phase 5: CLI Integration
- [x] `kairos ml train` command
- [x] `kairos ml list-models` command
- [x] `kairos ml validate-model` command
- [x] Configuration override via CLI arguments

#### Phase 6: Examples & Documentation
- [x] `train_simple_model.rs` example
- [x] `ml_strategy_backtest.rs` example
- [x] Comprehensive README with installation, usage, and troubleshooting
- [x] Performance benchmarks (structure only)

#### Phase 7: Testing & Polish
- [x] Integration tests for Phase 1-4
- [x] Clippy and format compliance

### Dependencies

```toml
tch = "0.23"           # PyTorch bindings
serde = "1"             # Serialization
serde_json = "1"
anyhow = "1"
thiserror = "1"
log = "0.4"
tracing = "0.1"
kairos-backtest = "0.1" # Internal dependency
kairos-data = "0.1"     # Internal dependency
kairos-study = "0.1"    # Internal dependency
```

### Known Limitations

- libtorch is required for building and runtime
- GPU training requires CUDA-enabled libtorch
- ONNX support is available but not fully integrated

## [0.1.0] - 2026-03-21

### Added
- Initial release with core ML strategy functionality
- Model training and inference
- Feature extraction from study outputs
- CLI commands for model management
