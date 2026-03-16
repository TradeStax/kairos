//! Web search tool using Brave Search API.

use super::ToolExecResult;
use serde_json::{Value, json};

const MAX_RESULTS: usize = 5;

/// Build the tool definition JSON for web_search.
pub fn tool_definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "web_search",
            "description": "Search the web for current market news, \
                economic events, or trading-related information. \
                Use for questions about recent events, earnings, \
                economic data releases, or market context.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (required)"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Max results (1-5, default 3)"
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }
        }
    })
}

/// Execute a web search against the Brave Search API.
pub async fn exec_web_search(
    query: &str,
    max_results: usize,
    api_key: &str,
    client: &reqwest::Client,
) -> ToolExecResult {
    let count = max_results.clamp(1, MAX_RESULTS);

    let response = client
        .get("https://api.search.brave.com/res/v1/web/search")
        .header("X-Subscription-Token", api_key)
        .header("Accept", "application/json")
        .query(&[("q", query), ("count", &count.to_string())])
        .send()
        .await;

    let response = match response {
        Ok(r) => r,
        Err(e) => {
            return ToolExecResult {
                content_json: json!({
                    "error": format!("Search request failed: {e}")
                })
                .to_string(),
                display_summary: "Search failed".to_string(),
                is_error: true,
            };
        }
    };

    if !response.status().is_success() {
        return ToolExecResult {
            content_json: json!({
                "error": format!(
                    "Search API error: {}",
                    response.status()
                )
            })
            .to_string(),
            display_summary: "Search API error".to_string(),
            is_error: true,
        };
    }

    let body: Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            return ToolExecResult {
                content_json: json!({
                    "error": format!(
                        "Failed to parse search results: {e}"
                    )
                })
                .to_string(),
                display_summary: "Parse error".to_string(),
                is_error: true,
            };
        }
    };

    let results: Vec<Value> = body["web"]["results"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .take(count)
                .map(|r| {
                    json!({
                        "title": r["title"].as_str()
                            .unwrap_or(""),
                        "description": r["description"].as_str()
                            .unwrap_or(""),
                        "url": r["url"].as_str()
                            .unwrap_or(""),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let short_q = if query.len() > 30 {
        &query[..30]
    } else {
        query
    };
    let summary = format!("{} results for \"{}\"", results.len(), short_q,);

    ToolExecResult {
        content_json: json!({
            "query": query,
            "results": results,
            "count": results.len(),
        })
        .to_string(),
        display_summary: summary,
        is_error: false,
    }
}
