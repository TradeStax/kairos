//! Margin calculation for futures positions.
//!
//! Futures exchanges require traders to post margin (collateral) for
//! each open position. This module enforces two margin tiers:
//!
//! - **Initial margin** -- the amount required to *open* a new
//!   position. Checked before order submission.
//! - **Maintenance margin** -- the amount required to *hold* an
//!   existing position. Used when computing available buying power.
//!
//! Per-instrument margin values come from [`InstrumentSpec`] but can
//! be overridden globally via the calculator's constructor.

use crate::config::instrument::InstrumentSpec;
use kairos_data::FuturesTicker;
use std::collections::HashMap;

/// Computes margin requirements for orders and positions.
///
/// Supports optional global overrides that take precedence over
/// per-instrument margin values from [`InstrumentSpec`].
pub struct MarginCalculator {
    /// If set, overrides the initial margin for all instruments.
    initial_override: Option<f64>,
    /// If set, overrides the maintenance margin for all instruments.
    maintenance_override: Option<f64>,
}

impl MarginCalculator {
    /// Create a new margin calculator with optional global overrides.
    ///
    /// Pass `None` for either parameter to fall back to the
    /// per-instrument values defined in [`InstrumentSpec`].
    #[must_use]
    pub fn new(initial_override: Option<f64>, maintenance_override: Option<f64>) -> Self {
        Self {
            initial_override,
            maintenance_override,
        }
    }

    /// Margin required to place a new order.
    ///
    /// Uses the **initial** margin rate (higher tier) since the
    /// position is not yet established.
    #[must_use]
    pub fn order_margin(
        &self,
        quantity: f64,
        instrument: &FuturesTicker,
        instruments: &HashMap<FuturesTicker, InstrumentSpec>,
    ) -> f64 {
        self.resolve_margin(quantity, instrument, instruments, true)
    }

    /// Margin required to maintain an existing position.
    ///
    /// Uses the **maintenance** margin rate (lower tier) since the
    /// position is already open.
    #[must_use]
    pub fn position_margin(
        &self,
        quantity: f64,
        instrument: &FuturesTicker,
        instruments: &HashMap<FuturesTicker, InstrumentSpec>,
    ) -> f64 {
        self.resolve_margin(quantity, instrument, instruments, false)
    }

    /// Resolve the appropriate margin value and multiply by quantity.
    ///
    /// When `use_initial` is true the initial-margin override/spec
    /// is used; otherwise the maintenance-margin one. Falls back to
    /// zero if no margin is configured for the instrument.
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
