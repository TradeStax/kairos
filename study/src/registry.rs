use crate::traits::{Study, StudyCategory, StudyPlacement};
use std::collections::HashMap;

/// Information about a registered study.
#[derive(Debug, Clone)]
pub struct StudyInfo {
    pub id: String,
    pub name: String,
    pub category: StudyCategory,
    pub placement: StudyPlacement,
    pub description: String,
}

/// Registry of study factories. Creates study instances by id.
pub struct StudyRegistry {
    factories: HashMap<String, Box<dyn Fn() -> Box<dyn Study> + Send + Sync>>,
    info: HashMap<String, StudyInfo>,
}

impl StudyRegistry {
    /// Create a new registry with all built-in studies registered.
    pub fn new() -> Self {
        let mut registry = Self {
            factories: HashMap::new(),
            info: HashMap::new(),
        };

        // Volume studies
        registry.register(
            "volume",
            StudyInfo {
                id: "volume".to_string(),
                name: "Volume".to_string(),
                category: StudyCategory::Volume,
                placement: StudyPlacement::Panel,
                description: "Total volume per candle".to_string(),
            },
            || Box::new(crate::volume::VolumeStudy::new()),
        );

        registry.register(
            "delta",
            StudyInfo {
                id: "delta".to_string(),
                name: "Volume Delta".to_string(),
                category: StudyCategory::Volume,
                placement: StudyPlacement::Panel,
                description: "Buy minus sell volume per candle".to_string(),
            },
            || Box::new(crate::volume::DeltaStudy::new()),
        );

        // Order flow studies
        registry.register(
            "volume_profile",
            StudyInfo {
                id: "volume_profile".to_string(),
                name: "Volume Profile".to_string(),
                category: StudyCategory::OrderFlow,
                placement: StudyPlacement::Background,
                description: "Volume distribution across price levels".to_string(),
            },
            || Box::new(crate::orderflow::VolumeProfileStudy::new()),
        );

        registry.register(
            "poc",
            StudyInfo {
                id: "poc".to_string(),
                name: "Point of Control".to_string(),
                category: StudyCategory::OrderFlow,
                placement: StudyPlacement::Overlay,
                description: "Price level with highest volume".to_string(),
            },
            || Box::new(crate::orderflow::PocStudy::new()),
        );

        registry.register(
            "value_area",
            StudyInfo {
                id: "value_area".to_string(),
                name: "Value Area".to_string(),
                category: StudyCategory::OrderFlow,
                placement: StudyPlacement::Background,
                description: "Value Area High and Low bands".to_string(),
            },
            || Box::new(crate::orderflow::ValueAreaStudy::new()),
        );

        registry.register(
            "imbalance",
            StudyInfo {
                id: "imbalance".to_string(),
                name: "Imbalance".to_string(),
                category: StudyCategory::OrderFlow,
                placement: StudyPlacement::Background,
                description: "Price levels with significant buy/sell imbalance".to_string(),
            },
            || Box::new(crate::orderflow::ImbalanceStudy::new()),
        );

        registry.register(
            "big_trades",
            StudyInfo {
                id: "big_trades".to_string(),
                name: "Big Trades".to_string(),
                category: StudyCategory::OrderFlow,
                placement: StudyPlacement::Overlay,
                description: "Aggregated institutional-scale trade bubbles".to_string(),
            },
            || Box::new(crate::orderflow::BigTradesStudy::new()),
        );

        // Trend studies
        registry.register(
            "sma",
            StudyInfo {
                id: "sma".to_string(),
                name: "Simple Moving Average".to_string(),
                category: StudyCategory::Trend,
                placement: StudyPlacement::Overlay,
                description: "Simple moving average of price".to_string(),
            },
            || Box::new(crate::trend::sma::SmaStudy::new()),
        );

        registry.register(
            "ema",
            StudyInfo {
                id: "ema".to_string(),
                name: "Exponential Moving Average".to_string(),
                category: StudyCategory::Trend,
                placement: StudyPlacement::Overlay,
                description: "Exponential moving average of price".to_string(),
            },
            || Box::new(crate::trend::ema::EmaStudy::new()),
        );

        registry.register(
            "vwap",
            StudyInfo {
                id: "vwap".to_string(),
                name: "Volume Weighted Average Price".to_string(),
                category: StudyCategory::Trend,
                placement: StudyPlacement::Overlay,
                description: "Volume weighted average price with optional bands".to_string(),
            },
            || Box::new(crate::trend::vwap::VwapStudy::new()),
        );

        // Volume studies
        registry.register(
            "cvd",
            StudyInfo {
                id: "cvd".to_string(),
                name: "Cumulative Volume Delta".to_string(),
                category: StudyCategory::Volume,
                placement: StudyPlacement::Panel,
                description: "Cumulative sum of buy minus sell volume".to_string(),
            },
            || Box::new(crate::volume::CvdStudy::new()),
        );

        registry.register(
            "obv",
            StudyInfo {
                id: "obv".to_string(),
                name: "On Balance Volume".to_string(),
                category: StudyCategory::Volume,
                placement: StudyPlacement::Panel,
                description: "Cumulative volume based on price direction".to_string(),
            },
            || Box::new(crate::volume::ObvStudy::new()),
        );

        // Volatility studies
        registry.register(
            "atr",
            StudyInfo {
                id: "atr".to_string(),
                name: "Average True Range".to_string(),
                category: StudyCategory::Volatility,
                placement: StudyPlacement::Panel,
                description: "Average true range using Wilder's smoothing".to_string(),
            },
            || Box::new(crate::volatility::atr::AtrStudy::new()),
        );

        // Volatility studies (continued)
        registry.register(
            "bollinger",
            StudyInfo {
                id: "bollinger".to_string(),
                name: "Bollinger Bands".to_string(),
                category: StudyCategory::Volatility,
                placement: StudyPlacement::Overlay,
                description: "SMA with standard deviation bands".to_string(),
            },
            || Box::new(crate::volatility::bollinger::BollingerStudy::new()),
        );

        // Momentum studies
        registry.register(
            "rsi",
            StudyInfo {
                id: "rsi".to_string(),
                name: "Relative Strength Index".to_string(),
                category: StudyCategory::Momentum,
                placement: StudyPlacement::Panel,
                description: "Momentum oscillator measuring overbought/oversold conditions"
                    .to_string(),
            },
            || Box::new(crate::momentum::rsi::RsiStudy::new()),
        );

        registry.register(
            "macd",
            StudyInfo {
                id: "macd".to_string(),
                name: "MACD".to_string(),
                category: StudyCategory::Momentum,
                placement: StudyPlacement::Panel,
                description: "Moving Average Convergence Divergence".to_string(),
            },
            || Box::new(crate::momentum::macd::MacdStudy::new()),
        );

        registry.register(
            "stochastic",
            StudyInfo {
                id: "stochastic".to_string(),
                name: "Stochastic Oscillator".to_string(),
                category: StudyCategory::Momentum,
                placement: StudyPlacement::Panel,
                description: "Stochastic oscillator with %K and %D lines".to_string(),
            },
            || Box::new(crate::momentum::stochastic::StochasticStudy::new()),
        );

        registry
    }

    /// Register a study factory.
    pub fn register<F>(&mut self, id: &str, info: StudyInfo, factory: F)
    where
        F: Fn() -> Box<dyn Study> + Send + Sync + 'static,
    {
        self.factories.insert(id.to_string(), Box::new(factory));
        self.info.insert(id.to_string(), info);
    }

    /// Check if a study with the given ID is already registered.
    pub fn contains(&self, id: &str) -> bool {
        self.factories.contains_key(id)
    }

    /// Create a study instance by id.
    pub fn create(&self, id: &str) -> Option<Box<dyn Study>> {
        self.factories.get(id).map(|f| f())
    }

    /// List all registered studies.
    pub fn list(&self) -> Vec<StudyInfo> {
        let mut studies: Vec<_> = self.info.values().cloned().collect();
        studies.sort_by(|a, b| a.name.cmp(&b.name));
        studies
    }

    /// List studies filtered by category.
    pub fn list_by_category(&self, category: StudyCategory) -> Vec<StudyInfo> {
        let mut studies: Vec<_> = self.info
            .values()
            .filter(|info| info.category == category)
            .cloned()
            .collect();
        studies.sort_by(|a, b| a.name.cmp(&b.name));
        studies
    }
}

impl Default for StudyRegistry {
    fn default() -> Self {
        Self::new()
    }
}
