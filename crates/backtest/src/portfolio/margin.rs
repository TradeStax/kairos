use crate::config::instrument::InstrumentSpec;
use kairos_data::FuturesTicker;
use std::collections::HashMap;

/// Computes margin requirements for orders and positions.
pub struct MarginCalculator {
    initial_override: Option<f64>,
    maintenance_override: Option<f64>,
}

impl MarginCalculator {
    pub fn new(initial_override: Option<f64>, maintenance_override: Option<f64>) -> Self {
        Self {
            initial_override,
            maintenance_override,
        }
    }

    /// Margin required to place a new order.
    pub fn order_margin(
        &self,
        quantity: f64,
        instrument: &FuturesTicker,
        instruments: &HashMap<FuturesTicker, InstrumentSpec>,
    ) -> f64 {
        self.resolve_margin(quantity, instrument, instruments, true)
    }

    /// Margin required to maintain an existing position.
    pub fn position_margin(
        &self,
        quantity: f64,
        instrument: &FuturesTicker,
        instruments: &HashMap<FuturesTicker, InstrumentSpec>,
    ) -> f64 {
        self.resolve_margin(quantity, instrument, instruments, false)
    }

    /// Shared helper: resolve the appropriate margin value and multiply
    /// by quantity. When `use_initial` is true the initial-margin
    /// override/spec is used; otherwise the maintenance-margin one.
    fn resolve_margin(
        &self,
        quantity: f64,
        instrument: &FuturesTicker,
        instruments: &HashMap<FuturesTicker, InstrumentSpec>,
        use_initial: bool,
    ) -> f64 {
        let margin = if use_initial {
            self.initial_override
                .or_else(|| instruments.get(instrument).and_then(|i| i.initial_margin))
        } else {
            self.maintenance_override.or_else(|| {
                instruments
                    .get(instrument)
                    .and_then(|i| i.maintenance_margin)
            })
        };
        quantity * margin.unwrap_or(0.0)
    }
}
