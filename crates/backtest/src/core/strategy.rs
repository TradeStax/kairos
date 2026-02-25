use crate::core::input::StrategyInput;
use crate::core::metadata::StrategyMetadata;
use crate::core::signal::Signal;
use kairos_data::Candle;
use kairos_study::{ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use thiserror::Error;

/// Errors produced by the backtest engine or strategy validation.
#[derive(Debug, Error)]
pub enum BacktestError {
    #[error("Invalid parameter '{key}': {reason}")]
    InvalidParameter { key: String, reason: String },
    #[error("Unknown strategy: {0}")]
    UnknownStrategy(String),
    #[error("Data error: {0}")]
    Data(String),
    #[error("Engine error: {0}")]
    Engine(String),
}

impl BacktestError {
    pub fn invalid_param(key: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidParameter { key: key.into(), reason: reason.into() }
    }
}

/// Core trait all backtest strategies must implement.
pub trait BacktestStrategy: Send + Sync {
    fn id(&self) -> &str;
    fn metadata(&self) -> StrategyMetadata;
    fn parameters(&self) -> &[ParameterDef];
    fn config(&self) -> &StudyConfig;
    fn config_mut(&mut self) -> &mut StudyConfig;

    /// Update a single parameter value with validation.
    fn set_parameter(&mut self, key: &str, value: ParameterValue) -> Result<(), BacktestError> {
        let def = self
            .parameters()
            .iter()
            .find(|p| p.key == key)
            .ok_or_else(|| BacktestError::invalid_param(key, "not found"))?;

        match (&def.kind, &value) {
            (ParameterKind::Integer { min, max }, ParameterValue::Integer(v)) => {
                if *v < *min || *v > *max {
                    return Err(BacktestError::invalid_param(
                        key,
                        format!("value {v} out of range [{min}, {max}]"),
                    ));
                }
            }
            (ParameterKind::Float { min, max, .. }, ParameterValue::Float(v)) => {
                if !v.is_finite() || *v < *min || *v > *max {
                    return Err(BacktestError::invalid_param(
                        key,
                        format!("value {v} out of range [{min}, {max}]"),
                    ));
                }
            }
            (ParameterKind::Boolean, ParameterValue::Boolean(_)) => {}
            (ParameterKind::Color, ParameterValue::Color(_)) => {}
            (ParameterKind::Choice { options }, ParameterValue::Choice(s)) => {
                if !options.contains(&s.as_str()) {
                    return Err(BacktestError::invalid_param(
                        key,
                        format!("invalid choice '{s}'"),
                    ));
                }
            }
            _ => {
                return Err(BacktestError::invalid_param(key, "type mismatch"));
            }
        }

        self.config_mut().set(key, value);
        Ok(())
    }

    /// Called once when the RTH session opens (first trade >= rth_open_hhmm).
    fn on_session_open(&mut self, input: &StrategyInput<'_>) -> Vec<Signal>;

    /// Called after each completed candle is closed.
    fn on_candle_close(&mut self, candle: &Candle, input: &StrategyInput<'_>) -> Vec<Signal>;

    /// Called on every trade tick during an open RTH session.
    fn on_tick(&mut self, input: &StrategyInput<'_>) -> Vec<Signal>;

    /// Called when the RTH session closes (first trade >= rth_close_hhmm).
    fn on_session_close(&mut self, input: &StrategyInput<'_>) -> Vec<Signal>;

    /// Reset all internal state (called between runs or on rewind).
    fn reset(&mut self);

    /// Deep-clone this strategy into a new box.
    fn clone_strategy(&self) -> Box<dyn BacktestStrategy>;
}
