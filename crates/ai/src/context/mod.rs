//! Context building — system prompts and chart data formatting.
//!
//! The context module assembles the system message and
//! structured chart overview that seed every AI conversation.

pub mod chart;
pub mod market;
mod system_prompt;

use crate::tools::ToolContext;

/// Context builder utilities.
pub struct ContextBuilder;

impl ContextBuilder {
    /// Build the system message for a conversation.
    ///
    /// Includes the base system prompt plus instrument-specific
    /// context from the provided ticker info.
    pub fn build_system_message(
        ticker_info: &data::FuturesTickerInfo,
    ) -> String {
        let base = system_prompt::SYSTEM_PROMPT;
        let ticker_ctx = format!(
            "\n\n## Current Instrument\n\
             - Symbol: {symbol}\n\
             - Product: {product}\n\
             - Venue: {venue}\n\
             - Tick size: {tick}\n\
             - Contract: {contract}",
            symbol = ticker_info.ticker.as_str(),
            product = ticker_info.ticker.product(),
            venue = ticker_info.ticker.venue,
            tick = ticker_info.tick_size,
            contract = ticker_info.ticker.contract_type(),
        );
        format!("{base}{ticker_ctx}")
    }

    /// Build a compact chart context string from the current
    /// tool context, suitable for inclusion in the system or
    /// first user message.
    pub fn build_chart_context(
        context: &ToolContext,
    ) -> String {
        chart::format_chart_overview(context)
    }
}
