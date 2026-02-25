//! Tool registry, execution, and definitions.
//!
//! Tools allow the AI assistant to inspect chart data,
//! compute studies, place annotations, and modify chart settings.

pub mod annotation;
pub mod candles;
pub mod chart_action;
pub mod depth;
mod pattern;
pub mod schema;
mod screenshot;
pub mod study;
pub mod trades;

use crate::error::AiError;

/// The runtime context provided to every tool execution.
///
/// Contains all chart data, drawings, and study outputs that
/// the tool can read from.
pub struct ToolContext {
    pub candles: Vec<data::Candle>,
    pub trades: Option<Vec<data::Trade>>,
    pub depth_snapshots: Option<Vec<data::DepthSnapshot>>,
    pub chart_config: data::ChartConfig,
    pub ticker_info: data::FuturesTickerInfo,
    pub drawings: Vec<data::SerializableDrawing>,
    pub study_outputs: Vec<(String, study::StudyOutput)>,
    pub tick_size: data::Price,
}

/// An action the AI wants to perform on the chart.
///
/// These are returned from tool execution and the app layer
/// is responsible for applying them.
#[derive(Debug, Clone)]
pub enum AiChartAction {
    /// Add a drawing/annotation to the chart.
    AddDrawing(data::SerializableDrawing),
    /// Add a study to the chart.
    AddStudy {
        study_id: String,
        params: Option<serde_json::Value>,
    },
    /// Remove a study by ID.
    RemoveStudy { study_id: String },
    /// Change the chart timeframe.
    ChangeTimeframe(data::Timeframe),
    /// Zoom the chart to a specific time range.
    ZoomToRange { start: u64, end: u64 },
}

/// Result from executing a single tool.
pub struct ToolExecutionResult {
    /// Text content to return to the model as the tool result.
    pub content: String,
    /// Zero or more chart actions to apply.
    pub chart_actions: Vec<AiChartAction>,
}

/// Stateless tool executor.
pub struct ToolExecutor;

impl ToolExecutor {
    /// Execute a tool by name with JSON arguments.
    pub fn execute(
        name: &str,
        arguments: &str,
        context: &ToolContext,
    ) -> Result<ToolExecutionResult, AiError> {
        match name {
            "get_candles" => {
                candles::execute_get_candles(
                    arguments, context,
                )
            }
            "get_trades" => {
                trades::execute_get_trades(
                    arguments, context,
                )
            }
            "get_depth_snapshot" => {
                depth::execute_get_depth(
                    arguments, context,
                )
            }
            "compute_study" => {
                self::study::execute_compute_study(
                    arguments, context,
                )
            }
            "add_annotation" => {
                annotation::execute_add_annotation(
                    arguments, context,
                )
            }
            "modify_chart" => {
                chart_action::execute_modify_chart(
                    arguments, context,
                )
            }
            "get_chart_screenshot" => {
                screenshot::execute_screenshot(
                    arguments, context,
                )
            }
            "search_similar_setups" => {
                pattern::execute_pattern_search(
                    arguments, context,
                )
            }
            _ => Err(AiError::ToolExecution {
                tool: name.to_string(),
                message: "unknown tool".to_string(),
            }),
        }
    }
}
