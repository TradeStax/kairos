//! # ML Strategy Module
//!
//! This module provides the [`MlStrategy`] struct that wraps ML models
//! behind the [`Strategy`](kairos_backtest::Strategy) trait, enabling
//! ML-based trading signals in the backtest engine.

pub mod config;

pub use config::MlStrategyConfig;

use crate::features::{FeatureExtractor, StudyFeatureExtractor};
use crate::model::{Model, ModelOutput, TradingSignal};
use kairos_backtest::order::request::OrderRequest;
use kairos_backtest::order::types::{OrderSide, OrderType};
use kairos_backtest::strategy::metadata::StrategyMetadata;
use kairos_backtest::strategy::{OrderEvent, Strategy as BacktestStrategy, StrategyContext};
use kairos_data::{Candle, FuturesTicker, FuturesVenue, Timeframe};
use kairos_study::{ParameterDef, StudyConfig};
use std::sync::Arc;

/// Signal generation helper
fn compute_trading_signal(
    probabilities: &[f64; 3],
    threshold_long: f64,
    threshold_short: f64,
) -> TradingSignal {
    if probabilities[0] >= threshold_long {
        TradingSignal::Long
    } else if probabilities[2] >= threshold_short {
        TradingSignal::Short
    } else {
        TradingSignal::Neutral
    }
}

/// ML Strategy that uses a trained model for signal generation
pub struct MlStrategy {
    /// Unique identifier
    id: String,
    /// Strategy metadata
    metadata: StrategyMetadata,
    /// Parameter configuration
    config: MlStrategyConfig,
    /// Study configuration (derived from feature config)
    study_config: StudyConfig,
    /// The ML model
    model: Option<Arc<dyn Model + Send + Sync>>,
    /// Feature extractor for study-to-tensor conversion
    feature_extractor: StudyFeatureExtractor,
    /// Whether warmup is complete
    warmup_complete: bool,
    /// Current signal from last prediction
    current_signal: TradingSignal,
    /// Current confidence from last prediction
    current_confidence: f64,
    /// Bar count for warmup tracking
    bars_processed: usize,
    /// Latest extracted features for inspection
    latest_features: Option<Vec<Vec<f64>>>,
}

impl MlStrategy {
    /// Create a new ML strategy
    pub fn new(config: MlStrategyConfig) -> Self {
        let id = config
            .id
            .clone()
            .unwrap_or_else(|| "ml_strategy".to_string());

        let metadata = StrategyMetadata {
            id: id.clone(),
            name: config
                .name
                .clone()
                .unwrap_or_else(|| "ML Strategy".to_string()),
            description: config.description.clone().unwrap_or_else(|| {
                "Machine learning-based strategy using trained models".to_string()
            }),
            category: kairos_backtest::strategy::metadata::StrategyCategory::Custom,
            version: "1.0.0",
        };

        // Create study config with strategy ID
        let study_config = StudyConfig::new(id.clone());

        // Create feature extractor
        let feature_extractor = StudyFeatureExtractor::new(config.feature_config.clone());

        Self {
            id,
            metadata,
            config,
            study_config,
            model: None,
            feature_extractor,
            warmup_complete: false,
            current_signal: TradingSignal::Neutral,
            current_confidence: 0.0,
            bars_processed: 0,
            latest_features: None,
        }
    }

    /// Set the model (used during initialization)
    pub fn set_model(&mut self, model: Arc<dyn Model + Send + Sync>) {
        self.model = Some(model);
    }

    /// Get the current signal
    pub fn current_signal(&self) -> TradingSignal {
        self.current_signal
    }

    /// Get the current confidence
    pub fn current_confidence(&self) -> f64 {
        self.current_confidence
    }

    /// Check if warmup is complete
    pub fn warmup_complete(&self) -> bool {
        self.warmup_complete
    }

    /// Get bars processed count
    pub fn bars_processed(&self) -> usize {
        self.bars_processed
    }

    /// Get the latest extracted features
    pub fn latest_features(&self) -> Option<&Vec<Vec<f64>>> {
        self.latest_features.as_ref()
    }
}

impl BacktestStrategy for MlStrategy {
    fn id(&self) -> &str {
        &self.id
    }

    fn metadata(&self) -> StrategyMetadata {
        self.metadata.clone()
    }

    fn parameters(&self) -> &[ParameterDef] {
        &[]
    }

    fn config(&self) -> &StudyConfig {
        &self.study_config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.study_config
    }

    fn required_studies(&self) -> Vec<kairos_backtest::strategy::StudyRequest> {
        self.config
            .feature_config
            .required_studies()
            .iter()
            .map(|key| kairos_backtest::strategy::StudyRequest {
                key: key.to_string(),
                study_id: key.to_string(), // Assume study ID matches key for now
                params: vec![],
            })
            .collect()
    }

    fn on_init(&mut self, _ctx: &StrategyContext) {
        // Model loading is handled externally via set_model()
        // This allows for more flexible model initialization
        log::info!("ML Strategy initialized: {}", self.id);
    }

    fn on_warmup_complete(&mut self, _ctx: &StrategyContext) {
        self.warmup_complete = true;
        log::info!(
            "ML Strategy warmup complete after {} bars",
            self.bars_processed
        );
    }

    fn on_session_open(&mut self, _ctx: &StrategyContext) -> Vec<OrderRequest> {
        // Reset daily state if needed
        vec![]
    }

    fn on_candle(
        &mut self,
        _instrument: FuturesTicker,
        _timeframe: Timeframe,
        _candle: &Candle,
        ctx: &StrategyContext,
    ) -> Vec<OrderRequest> {
        self.bars_processed += 1;

        // Extract features from studies
        let mut features: Vec<Vec<f64>> = Vec::new();

        for feature_def in &self.config.feature_config.features {
            if let Some(study_output) = ctx.studies.get(&feature_def.study_key) {
                // Extract values from study output
                if let Some(values) =
                    self.extract_study_values(study_output, &feature_def.output_field)
                {
                    // Apply transform if specified
                    let values = if let Some(transform) = feature_def.transform {
                        transform.apply(&values)
                    } else {
                        values
                    };
                    features.push(values);
                }
            }
        }

        // If we don't have enough features yet, return neutral
        if features.len() != self.config.feature_config.features.len() {
            return vec![];
        }

        // Check warmup period
        let lookback = self.config.feature_config.lookback_periods;
        if features.iter().any(|f| f.len() < lookback) {
            return vec![];
        }

        // Mark warmup complete after first full extraction
        if !self.warmup_complete && self.bars_processed >= lookback {
            self.warmup_complete = true;
            log::info!(
                "ML Strategy warmup complete after {} bars",
                self.bars_processed
            );
        }

        // If model not loaded, return neutral
        let model = match &self.model {
            Some(m) => m,
            None => return vec![],
        };

        // Store latest features for inspection
        self.latest_features = Some(features.clone());

        // Run inference
        match self.run_inference(&features, model) {
            Ok(output) => {
                self.current_signal = output.signal();
                self.current_confidence = output.confidence();

                // Generate order request based on signal
                self.generate_order_request(output)
            }
            Err(e) => {
                log::warn!("ML inference error: {}", e);
                vec![]
            }
        }
    }

    fn on_tick(&mut self, _ctx: &StrategyContext) -> Vec<OrderRequest> {
        // ML strategy operates on candle data, not tick data
        vec![]
    }

    fn on_session_close(&mut self, _ctx: &StrategyContext) -> Vec<OrderRequest> {
        // Flatten positions at session close
        vec![]
    }

    fn on_order_event(&mut self, _event: OrderEvent, _ctx: &StrategyContext) -> Vec<OrderRequest> {
        vec![]
    }

    fn reset(&mut self) {
        self.warmup_complete = false;
        self.current_signal = TradingSignal::Neutral;
        self.current_confidence = 0.0;
        self.bars_processed = 0;
        self.feature_extractor.reset();
        self.latest_features = None;
    }

    fn clone_strategy(&self) -> Box<dyn BacktestStrategy> {
        let mut cloned = MlStrategy::new(self.config.clone());
        if let Some(model) = &self.model {
            cloned.model = Some(model.clone());
        }
        Box::new(cloned)
    }
}

impl MlStrategy {
    /// Extract values from a study output based on field path
    fn extract_study_values(
        &self,
        output: &kairos_study::StudyOutput,
        field_path: &str,
    ) -> Option<Vec<f64>> {
        // Simple extraction for Lines series
        match output {
            kairos_study::StudyOutput::Lines(lines) => {
                if field_path == "line" || field_path == "value" || field_path == "lines" {
                    // Return the last values from the first line
                    if let Some(line) = lines.first() {
                        return Some(line.points.iter().map(|(_, v)| *v as f64).collect());
                    }
                }
                // If there's a specific index like "lines.0"
                if let Some(idx) = field_path.strip_prefix("lines.")
                    && let Ok(index) = idx.parse::<usize>()
                    && let Some(line) = lines.get(index)
                {
                    return Some(line.points.iter().map(|(_, v)| *v as f64).collect());
                }
                // Try to find by label
                for line in lines {
                    if line.label == field_path {
                        return Some(line.points.iter().map(|(_, v)| *v as f64).collect());
                    }
                }
                None
            }
            kairos_study::StudyOutput::Band {
                upper,
                middle,
                lower,
                ..
            } => Some(match field_path {
                "band.upper" | "upper" => upper.points.iter().map(|(_, v)| *v as f64).collect(),
                "band.middle" | "middle" => middle
                    .as_ref()
                    .map(|m| m.points.iter().map(|(_, v)| *v as f64).collect())
                    .unwrap_or_default(),
                "band.lower" | "lower" => lower.points.iter().map(|(_, v)| *v as f64).collect(),
                _ => return None,
            }),
            kairos_study::StudyOutput::Bars(bars) => {
                if field_path == "bars" || field_path == "values" {
                    return Some(
                        bars.iter()
                            .flat_map(|b| b.points.iter().map(|p| p.value as f64))
                            .collect(),
                    );
                }
                if let Some(idx) = field_path.strip_prefix("bars.")
                    && let Ok(index) = idx.parse::<usize>()
                    && let Some(bar) = bars.get(index)
                {
                    return Some(bar.points.iter().map(|p| p.value as f64).collect());
                }
                None
            }
            kairos_study::StudyOutput::Histogram(bars) => {
                if field_path == "histogram" || field_path == "values" {
                    return Some(bars.iter().map(|b| b.value as f64).collect());
                }
                None
            }
            _ => None,
        }
    }

    /// Run model inference
    #[cfg(feature = "tch")]
    fn run_inference(
        &self,
        features: &[Vec<f64>],
        model: &Arc<dyn Model + Send + Sync>,
    ) -> Result<ModelOutput, String> {
        use tch::Tensor;

        // Transpose features: [features, lookback] -> [1, lookback, features]
        let lookback = self.config.feature_config.lookback_periods;
        let num_features = features.len();

        // Create input tensor [1, lookback, features]
        let mut data: Vec<f32> = Vec::with_capacity(lookback * num_features);

        for i in 0..lookback {
            for feature_values in features {
                if i < feature_values.len() {
                    data.push(feature_values[i] as f32);
                } else {
                    data.push(0.0f32);
                }
            }
        }

        let input = Tensor::from_slice(&data).reshape([1, lookback as i64, num_features as i64]);

        model.predict(&input).map_err(|e| e.to_string())
    }

    /// Run model inference (fallback without tch)
    #[cfg(not(feature = "tch"))]
    fn run_inference(
        &self,
        _features: &[Vec<f64>],
        _model: &Arc<dyn Model + Send + Sync>,
    ) -> Result<ModelOutput, String> {
        Err("ML inference requires the 'tch' feature".to_string())
    }

    /// Generate order request from model output
    fn generate_order_request(&self, output: ModelOutput) -> Vec<OrderRequest> {
        // Only generate orders when warmup is complete
        if !self.warmup_complete {
            return vec![];
        }

        // Check confidence threshold
        let min_confidence = self.config.min_confidence;
        if output.confidence() < min_confidence {
            return vec![];
        }

        // Create instrument for orders - use primary_instrument from context in real usage
        // For now, use a placeholder that will be overridden
        let instrument = FuturesTicker::new("NQ", FuturesVenue::CMEGlobex);

        match output {
            ModelOutput::Classification {
                probabilities,
                prediction: _,
            } => {
                let signal = compute_trading_signal(
                    &probabilities,
                    self.config.signal_threshold_long,
                    self.config.signal_threshold_short,
                );

                match signal {
                    TradingSignal::Long => {
                        vec![OrderRequest::Submit(
                            kairos_backtest::order::request::NewOrder {
                                instrument,
                                side: OrderSide::Buy,
                                order_type: OrderType::Market,
                                quantity: 1.0,
                                time_in_force: kairos_backtest::order::types::TimeInForce::GTC,
                                label: Some("ml_long".to_string()),
                                reduce_only: false,
                            },
                        )]
                    }
                    TradingSignal::Short => {
                        vec![OrderRequest::Submit(
                            kairos_backtest::order::request::NewOrder {
                                instrument,
                                side: OrderSide::Sell,
                                order_type: OrderType::Market,
                                quantity: 1.0,
                                time_in_force: kairos_backtest::order::types::TimeInForce::GTC,
                                label: Some("ml_short".to_string()),
                                reduce_only: false,
                            },
                        )]
                    }
                    TradingSignal::Neutral => vec![],
                }
            }
            ModelOutput::Regression { value } => {
                if value > self.config.signal_threshold_long {
                    vec![OrderRequest::Submit(
                        kairos_backtest::order::request::NewOrder {
                            instrument,
                            side: OrderSide::Buy,
                            order_type: OrderType::Market,
                            quantity: 1.0,
                            time_in_force: kairos_backtest::order::types::TimeInForce::GTC,
                            label: Some("ml_long".to_string()),
                            reduce_only: false,
                        },
                    )]
                } else if value < -self.config.signal_threshold_short {
                    vec![OrderRequest::Submit(
                        kairos_backtest::order::request::NewOrder {
                            instrument,
                            side: OrderSide::Sell,
                            order_type: OrderType::Market,
                            quantity: 1.0,
                            time_in_force: kairos_backtest::order::types::TimeInForce::GTC,
                            label: Some("ml_short".to_string()),
                            reduce_only: false,
                        },
                    )]
                } else {
                    vec![]
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{FeatureConfig, FeatureDefinition, NormalizationMethod};

    fn create_test_config() -> MlStrategyConfig {
        MlStrategyConfig {
            id: Some("test_ml_strategy".to_string()),
            name: Some("Test ML Strategy".to_string()),
            description: Some("A test ML strategy".to_string()),
            model_path: None,
            feature_config: FeatureConfig {
                features: vec![
                    FeatureDefinition::new("sma_20", "line"),
                    FeatureDefinition::new("rsi_14", "value"),
                ],
                lookback_periods: 20,
                normalization: NormalizationMethod::ZScore,
            },
            signal_threshold_long: 0.6,
            signal_threshold_short: 0.6,
            min_confidence: 0.5,
            use_confidence_for_sizing: false,
        }
    }

    #[test]
    fn test_ml_strategy_config_defaults() {
        let config = MlStrategyConfig::default();
        assert_eq!(config.signal_threshold_long, 0.6);
        assert_eq!(config.signal_threshold_short, 0.6);
        assert_eq!(config.min_confidence, 0.5);
    }

    #[test]
    fn test_ml_strategy_initializes_with_config() {
        let config = create_test_config();
        let strategy = MlStrategy::new(config);

        assert_eq!(strategy.id(), "test_ml_strategy");
        assert!(!strategy.warmup_complete()); // Should be false initially
        assert_eq!(strategy.bars_processed(), 0);
    }

    #[test]
    fn test_strategy_provides_required_studies() {
        let config = create_test_config();
        let strategy = MlStrategy::new(config);

        let studies = strategy.required_studies();
        assert_eq!(studies.len(), 2);
        assert!(studies.iter().any(|s| s.key == "sma_20"));
        assert!(studies.iter().any(|s| s.key == "rsi_14"));
    }

    #[test]
    fn test_strategy_has_parameters() {
        let config = create_test_config();
        let strategy = MlStrategy::new(config);

        // ML strategy uses config for parameters, not parameter defs
        let params = strategy.parameters();
        assert!(params.is_empty());
    }

    #[test]
    fn test_reset_clears_state() {
        let config = create_test_config();
        let mut strategy = MlStrategy::new(config);

        // Simulate some state
        strategy.bars_processed = 100;
        strategy.warmup_complete = true;

        strategy.reset();

        assert_eq!(strategy.bars_processed(), 0);
        assert!(!strategy.warmup_complete());
        assert_eq!(strategy.current_signal(), TradingSignal::Neutral);
    }

    #[test]
    fn test_clone_strategy_creates_independent_copy() {
        let config = create_test_config();
        let strategy = MlStrategy::new(config);

        let cloned = strategy.clone_strategy();

        // IDs should match
        assert_eq!(cloned.id(), strategy.id());
    }

    #[test]
    fn test_signal_generation_long_threshold() {
        let probabilities = [0.7, 0.2, 0.1]; // 70% long
        let signal = compute_trading_signal(&probabilities, 0.6, 0.6);
        assert_eq!(signal, TradingSignal::Long);
    }

    #[test]
    fn test_signal_generation_short_threshold() {
        let probabilities = [0.1, 0.2, 0.7]; // 70% short
        let signal = compute_trading_signal(&probabilities, 0.6, 0.6);
        assert_eq!(signal, TradingSignal::Short);
    }

    #[test]
    fn test_signal_generation_neutral_below_both() {
        let probabilities = [0.4, 0.3, 0.3]; // 40% long, below threshold
        let signal = compute_trading_signal(&probabilities, 0.6, 0.6);
        assert_eq!(signal, TradingSignal::Neutral);
    }

    #[test]
    fn test_signal_generation_at_exact_threshold() {
        let probabilities = [0.6, 0.2, 0.2]; // Exactly at threshold
        let signal = compute_trading_signal(&probabilities, 0.6, 0.6);
        assert_eq!(signal, TradingSignal::Long);
    }

    #[test]
    fn test_config_validation_rejects_invalid_threshold() {
        let mut config = MlStrategyConfig::default();

        // Invalid threshold (must be 0.0-1.0)
        config.signal_threshold_long = 1.5;
        let result = config.validate();
        assert!(result.is_err());

        config.signal_threshold_long = -0.1;
        let result = config.validate();
        assert!(result.is_err());
    }
}
