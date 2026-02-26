//! Content kind enum for pane types.

use data::ChartType;
use serde::{Deserialize, Serialize};

/// Content kind for a pane
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Starter,
    #[cfg(feature = "heatmap")]
    HeatmapChart,
    CandlestickChart,
    #[cfg(feature = "heatmap")]
    Ladder,
    ComparisonChart,
    ProfileChart,
    BacktestResult,
    AiAssistant,
}

// Custom Serialize that writes CandlestickChart as "CandlestickChart"
impl Serialize for ContentKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ContentKind::Starter => serializer.serialize_str("Starter"),
            #[cfg(feature = "heatmap")]
            ContentKind::HeatmapChart => serializer.serialize_str("HeatmapChart"),
            ContentKind::CandlestickChart => serializer.serialize_str("CandlestickChart"),
            #[cfg(feature = "heatmap")]
            ContentKind::Ladder => serializer.serialize_str("Ladder"),
            ContentKind::ComparisonChart => serializer.serialize_str("ComparisonChart"),
            ContentKind::ProfileChart => serializer.serialize_str("ProfileChart"),
            ContentKind::BacktestResult => serializer.serialize_str("BacktestResult"),
            ContentKind::AiAssistant => serializer.serialize_str("AiAssistant"),
        }
    }
}

// Custom Deserialize that maps "FootprintChart" -> CandlestickChart for backward compat
impl<'de> Deserialize<'de> for ContentKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "Starter" => Ok(ContentKind::Starter),
            #[cfg(feature = "heatmap")]
            "HeatmapChart" => Ok(ContentKind::HeatmapChart),
            #[cfg(not(feature = "heatmap"))]
            "HeatmapChart" => Ok(ContentKind::Starter),
            "CandlestickChart" | "FootprintChart" => Ok(ContentKind::CandlestickChart),
            "TimeAndSales" => Ok(ContentKind::Starter),
            #[cfg(feature = "heatmap")]
            "Ladder" => Ok(ContentKind::Ladder),
            #[cfg(not(feature = "heatmap"))]
            "Ladder" => Ok(ContentKind::Starter),
            "ComparisonChart" => Ok(ContentKind::ComparisonChart),
            "ScriptEditor" => Ok(ContentKind::Starter),
            "ProfileChart" => Ok(ContentKind::ProfileChart),
            "BacktestResult" => Ok(ContentKind::BacktestResult),
            "AiAssistant" => Ok(ContentKind::AiAssistant),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &[
                    "Starter",
                    "HeatmapChart",
                    "CandlestickChart",
                    "Ladder",
                    "ComparisonChart",
                    "ProfileChart",
                    "BacktestResult",
                    "AiAssistant",
                ],
            )),
        }
    }
}

impl ContentKind {
    pub const ALL: &'static [ContentKind] = &[
        #[cfg(feature = "heatmap")]
        ContentKind::HeatmapChart,
        ContentKind::CandlestickChart,
        ContentKind::ProfileChart,
        #[cfg(feature = "heatmap")]
        ContentKind::Ladder,
        ContentKind::ComparisonChart,
        ContentKind::AiAssistant,
    ];

    pub fn to_chart_type(self) -> ChartType {
        match self {
            #[cfg(feature = "heatmap")]
            ContentKind::HeatmapChart => ChartType::Heatmap,
            ContentKind::CandlestickChart => ChartType::Candlestick,
            #[cfg(feature = "heatmap")]
            ContentKind::Ladder => ChartType::Candlestick,
            ContentKind::ComparisonChart => ChartType::Candlestick,
            ContentKind::Starter => ChartType::Candlestick,
            ContentKind::ProfileChart => ChartType::Candlestick,
            ContentKind::BacktestResult => ChartType::Candlestick,
            ContentKind::AiAssistant => ChartType::Candlestick,
        }
    }
}

impl std::fmt::Display for ContentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentKind::Starter => write!(f, "Starter"),
            #[cfg(feature = "heatmap")]
            ContentKind::HeatmapChart => write!(f, "Heatmap"),
            ContentKind::CandlestickChart => write!(f, "Candlestick"),
            #[cfg(feature = "heatmap")]
            ContentKind::Ladder => write!(f, "Ladder"),
            ContentKind::ComparisonChart => write!(f, "Comparison"),
            ContentKind::ProfileChart => write!(f, "Profile"),
            ContentKind::BacktestResult => write!(f, "Backtest Result"),
            ContentKind::AiAssistant => write!(f, "AI Assistant"),
        }
    }
}
