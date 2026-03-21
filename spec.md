# ML Strategy Specification

## Project Name
**Kairos ML Strategy Module**

## Version
**1.0.0**

## Date Created
2026-03-21

## Status
Draft

---

## Problem Statement

Kairos currently supports only rule-based trading strategies built from classical technical indicators (SMA, EMA, RSI, VWAP, etc.). While these indicators are valuable, traders increasingly want to use machine learning models to:

1. Learn non-linear relationships between indicators and price movements
2. Combine multiple indicators with different timeframes as features
3. Adapt to changing market conditions
4. Find patterns that human-designed rules miss

This feature adds PyTorch-based ML model support to Kairos, enabling both inference during backtesting/live trading and model training on historical indicator data.

---

## Goals & Success Criteria

### Primary Goals
- Add a new `kairos-ml` crate with PyTorch integration via `tch`
- Enable strategies to use ML model predictions as trading signals
- Support training models using built-in indicators as features
- Integrate seamlessly with existing study/indicator system
- Maintain Rust's performance characteristics (no Python runtime)

### Success Metrics
- Models can be loaded from ONNX or PyTorch state dict format
- Feature extraction pipeline converts any combination of studies to model input
- Training produces models that improve on baseline (random/rule-based) performance
- Inference latency < 10ms per prediction during backtesting
- 100% compatibility with existing strategy lifecycle (on_init, on_candle, etc.)

### Non-Goals
- Real-time/live trading model serving (future work)
- Automated hyperparameter tuning (future work)
- Model versioning and A/B testing infrastructure (future work)
- Cloud-based training (local training only initially)

---

## Functional Requirements

### User Stories

1. **As a quantitative researcher**, I want to define a feature set by selecting studies (e.g., SMA(20), RSI(14), VWAP) so that I can train models that learn from these indicators.

2. **As a backtester**, I want to load a trained model and run it against historical data so that I can evaluate its performance.

3. **As a strategy developer**, I want to combine ML predictions with rule-based logic so that I can create hybrid strategies.

4. **As a trader**, I want the model to output directional signals (long/short/neutral) with confidence scores so that I can size positions accordingly.

### Core Features

#### 1. Model Loading
- **ONNX Import**: Load models exported from PyTorch via ONNX
- **State Dict Import**: Load PyTorch models directly via tch (requires compatible architecture)
- **Model Registry**: Centralized model file management

#### 2. Feature Extraction
- **Study-to-Feature Pipeline**: Convert any `StudyOutput` to normalized tensor values
- **Window Management**: Maintain rolling windows of features (lookback period)
- **Normalization**: Z-score normalization of features based on rolling statistics
- **Missing Data Handling**: Handle studies that don't have values at all bars

#### 3. Inference Engine
- **Batch Prediction**: Process multiple bars efficiently
- **Signal Output**: Convert model output to trade signals (long/short/neutral)
- **Confidence Scores**: Return prediction confidence for position sizing
- **Warm-up**: Initialize model with historical data before generating signals

#### 4. Training Pipeline
- **Data Generation**: Generate feature matrices from historical candle/study data
- **Label Generation**: Create training labels (e.g., N-bar forward return, direction)
- **Training Loop**: Mini-batch gradient descent with configurable hyperparameters
- **Validation**: Train/validation split with out-of-sample performance tracking
- **Model Export**: Save trained models in compatible format

#### 5. ML Strategy Trait
- **Strategy Integration**: New `MlStrategy` wrapper implementing `Strategy` trait
- **Parameter Configuration**: Define feature studies, lookback, model path via parameters
- **Lifecycle Management**: Handle model warm-up, inference, and reset

### User Interactions

1. **Configure Feature Studies**
   - Strategy parameters select which studies to use as features
   - Each study maps to one or more input features
   - Studies must complete warm-up before model can predict

2. **Load Model**
   - Provide path to ONNX file or PyTorch checkpoint
   - Model architecture must match feature configuration

3. **Training Workflow**
   - Select date range for training data
   - Configure label generation parameters (e.g., horizon, threshold)
   - Train model with progress feedback
   - Save trained model

4. **Backtesting**
   - Run backtest with ML strategy
   - View equity curve, trade log, and feature importance (if available)

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
│  │   Requests   │    │  Generator   │    │   Inference       │  │
│  └──────────────┘    └──────────────┘    └──────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Technical Requirements

### Languages & Frameworks
- **Primary Language**: Rust (2024 edition)
- **ML Framework**: PyTorch via `tch` crate
- **ONNX Runtime**: `ort` crate for ONNX models
- **Minimum Rust Version**: 1.75+

### Libraries & Dependencies

```toml
[dependencies]
tch = "0.15"           # PyTorch bindings
ort = "1.17"           # ONNX Runtime (optional, for ONNX support)
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "1"

[dev-dependencies]
tempfile = "3"
```

### Architecture Patterns

1. **Facade Pattern**: `MlStrategy` wraps model inference behind `Strategy` trait
2. **Builder Pattern**: Feature pipeline configured via builder
3. **Registry Pattern**: Model loading centralized in registry
4. **Trait Objects**: Model trait allows different implementations (tch, ort)

### Development Style
- Test-Driven Development (TDD) with unit tests
- Integration tests for full pipeline
- Performance benchmarks for inference latency

---

## Data Models

### FeatureConfig
```rust
pub struct FeatureConfig {
    /// Studies to extract as features, keyed by strategy study key
    pub features: Vec<FeatureDefinition>,
    /// Number of historical bars to include (lookback)
    pub lookback_periods: usize,
    /// Normalization method
    pub normalization: NormalizationMethod,
}

pub struct FeatureDefinition {
    /// Strategy study key (e.g., "sma_20")
    pub study_key: String,
    /// Which output field to use (e.g., "line", "band.upper")
    pub output_field: String,
    /// Optional transform (log, diff, pct_change)
    pub transform: Option<FeatureTransform>,
}
```

### ModelInput
```rust
/// Input tensor for model inference
/// Shape: [batch_size, lookback_periods, num_features]
pub struct ModelInput {
    pub tensor: tch::Tensor,
    pub timestamps: Vec<u64>,
    pub bar_indices: Vec<usize>,
}
```

### ModelOutput
```rust
pub enum ModelOutput {
    /// Classification: [long_prob, neutral_prob, short_prob]
    Classification {
        probabilities: [f64; 3],
        prediction: TradingSignal,
    },
    /// Regression: raw prediction value
    Regression {
        value: f64,
    },
}

pub enum TradingSignal {
    Long,
    Short,
    Neutral,
}
```

### TrainingConfig
```rust
pub struct TrainingConfig {
    /// Model architecture
    pub model_type: ModelType,
    /// Training hyperparameters
    pub learning_rate: f64,
    pub batch_size: usize,
    pub epochs: usize,
    pub optimizer: OptimizerType,
    /// Label generation
    pub label_horizon: usize,       // N bars forward
    pub label_threshold: f64,       // % threshold for long/short
    /// Validation
    pub validation_split: f64,
    pub early_stopping_patience: usize,
}
```

---

## Non-Functional Requirements

### Performance
- Inference latency: < 10ms per prediction on CPU
- Feature extraction: < 5ms for 100 studies per candle
- Training throughput: > 1000 samples/second on GPU

### Memory
- Model size limit: 500MB
- Feature buffer: Configurable, default 1000 bars
- Peak memory during training: < 8GB

### Reliability
- Graceful degradation if model fails to load
- Clear error messages for misconfigured features
- Fallback to neutral signal on inference errors

### Maintainability
- All public APIs documented with doc comments
- Unit test coverage > 80% for core modules
- Example strategies and training scripts

---

## Dependencies & Constraints

### External Dependencies
- PyTorch C library (bundled with tch)
- libtorch system library for GPU support (optional)

### Build Constraints
- tch requires C compiler (gcc/clang)
- Some platforms may require CUDA for GPU training
- ONNX Runtime adds ~50MB to binary size

### Team Constraints
- Initial implementation by ML-focused developer
- Code review by Rust developer for safety
- Testing with real market data required

---

## Risks & Mitigation

### Technical Risks
1. **tch API instability**: `tch` closely mirrors PyTorch API; breaking changes possible
   - *Mitigation*: Pin to specific version, test against new releases

2. **Model compatibility**: Users may have PyTorch models that don't work with tch
   - *Mitigation*: Support both ONNX (portable) and tch (native) formats

3. **Feature engineering complexity**: Different study outputs have different shapes
   - *Mitigation*: Abstract behind Feature trait, handle each study type explicitly

4. **Training convergence**: Models may not learn meaningful patterns
   - *Mitigation*: Start with simple architectures, provide baselines to beat

### Resource Risks
1. **Build time**: tch adds significant compile time
   - *Mitigation*: Feature-gate ML features, only compile when needed

2. **Binary size**: tch + ort increases binary by ~100MB
   - *Mitigation*: Provide feature flags for optional components

### Timeline Risks
1. **Feature complexity**: Indicator-to-tensor conversion may be more work than expected
   - *Mitigation*: Prioritize MVP with LineSeries only first

---

## Future Considerations

1. **GPU Training**: Add CUDA support for faster training
2. **AutoML**: Automated architecture search and hyperparameter tuning
3. **Model Market**: Repository of pre-trained models
4. **Real-time Serving**: Live trading integration
5. **Feature Store**: Pre-computed features for fast backtesting
