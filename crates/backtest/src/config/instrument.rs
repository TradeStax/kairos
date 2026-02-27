//! Instrument specification for backtest fill simulation.
//!
//! [`InstrumentSpec`] captures the contract-level details needed to
//! convert price ticks into dollar P&L: tick size, point multiplier,
//! and per-contract margin requirements. Known CME products are
//! pre-populated via [`InstrumentSpec::from_ticker`].

use kairos_data::{FuturesTicker, Price};
use serde::{Deserialize, Serialize};

/// Contract specification for a single futures instrument.
///
/// Used by the fill simulator and portfolio accounting to convert
/// price movements into dollar values and enforce margin
/// requirements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstrumentSpec {
    /// The futures ticker this spec describes.
    pub ticker: FuturesTicker,
    /// Minimum price increment (e.g. 0.25 for ES).
    pub tick_size: Price,
    /// Dollar value of one full contract point (e.g. $50 for ES,
    /// $20 for NQ).
    pub multiplier: f64,
    /// Dollar value of a single tick move.
    ///
    /// Computed as `tick_size.to_f64() * multiplier`.
    pub tick_value: f64,
    /// Initial margin per contract in USD, if known.
    pub initial_margin: Option<f64>,
    /// Maintenance margin per contract in USD, if known.
    pub maintenance_margin: Option<f64>,
}

impl InstrumentSpec {
    /// Creates a new spec with the given tick size and multiplier.
    ///
    /// `tick_value` is computed automatically. Margin fields are
    /// left as `None`; use [`with_margins`] to set them.
    ///
    /// [`with_margins`]: InstrumentSpec::with_margins
    #[must_use]
    pub fn new(ticker: FuturesTicker, tick_size: Price, multiplier: f64) -> Self {
        let tick_value = tick_size.to_f64() * multiplier;
        Self {
            ticker,
            tick_size,
            multiplier,
            tick_value,
            initial_margin: None,
            maintenance_margin: None,
        }
    }

    /// Sets initial and maintenance margin requirements.
    #[must_use]
    pub fn with_margins(mut self, initial: f64, maintenance: f64) -> Self {
        self.initial_margin = Some(initial);
        self.maintenance_margin = Some(maintenance);
        self
    }

    /// Builds an [`InstrumentSpec`] from a [`FuturesTicker`] using
    /// known CME product defaults.
    ///
    /// Supported products: ES, NQ, YM, RTY, GC, SI, CL, NG, HG,
    /// ZN, ZB, ZF. Unrecognized products fall back to ES defaults.
    #[must_use]
    pub fn from_ticker(ticker: FuturesTicker) -> Self {
        let product = ticker.product();
        let (tick_size, multiplier, initial, maintenance) = match product {
            "ES" => (0.25, 50.0, 15_900.0, 14_400.0),
            "NQ" => (0.25, 20.0, 21_000.0, 19_000.0),
            "YM" => (1.0, 5.0, 11_000.0, 10_000.0),
            "RTY" => (0.10, 50.0, 8_000.0, 7_200.0),
            "GC" => (0.10, 100.0, 11_000.0, 10_000.0),
            "SI" => (0.005, 5_000.0, 10_000.0, 9_000.0),
            "CL" => (0.01, 1_000.0, 7_000.0, 6_400.0),
            "NG" => (0.001, 10_000.0, 4_500.0, 4_100.0),
            "HG" => (0.0005, 25_000.0, 5_500.0, 5_000.0),
            "ZN" => (0.015625, 1_000.0, 2_200.0, 2_000.0),
            "ZB" => (0.03125, 1_000.0, 4_400.0, 4_000.0),
            "ZF" => (0.0078125, 1_000.0, 1_400.0, 1_300.0),
            _ => (0.25, 50.0, 15_900.0, 14_400.0),
        };
        Self::new(ticker, Price::from_f64(tick_size), multiplier).with_margins(initial, maintenance)
    }
}
