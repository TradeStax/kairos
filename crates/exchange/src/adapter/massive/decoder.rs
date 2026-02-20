use super::{MassiveError, MassiveResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Massive API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassiveResponse<T> {
    pub status: String,
    pub request_id: Option<String>,
    pub results: Option<T>,
    pub next_url: Option<String>,
    pub count: Option<usize>,
}

/// Option snapshot from Massive API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassiveOptionSnapshot {
    pub ticker: String,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub asset_type: Option<String>,
    pub market_status: Option<String>,
    pub timeframe: Option<String>,
    pub last_updated: Option<u64>, // Nanoseconds
    pub last_trade: Option<LastTrade>,
    pub last_quote: Option<LastQuote>,
    pub greeks: Option<Greeks>,
    pub implied_volatility: Option<f64>,
    pub open_interest: Option<u64>,
    pub break_even_price: Option<f64>,
    pub details: Option<ContractDetails>,
    pub underlying_asset: Option<UnderlyingAsset>,
    pub day: Option<DayBar>,
}

/// Last trade information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastTrade {
    pub conditions: Option<Vec<i32>>,
    pub exchange: Option<i32>,
    pub price: Option<f64>,
    pub sip_timestamp: Option<u64>, // Nanoseconds
    pub size: Option<f64>,
}

/// Last quote information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastQuote {
    pub ask: Option<f64>,
    pub ask_size: Option<f64>,
    pub bid: Option<f64>,
    pub bid_size: Option<f64>,
    pub last_updated: Option<u64>, // Nanoseconds
    pub timeframe: Option<String>,
}

/// Greek values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greeks {
    pub delta: Option<f64>,
    pub gamma: Option<f64>,
    pub theta: Option<f64>,
    pub vega: Option<f64>,
}

/// Contract details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDetails {
    pub contract_type: Option<String>,   // "call", "put", "other"
    pub exercise_style: Option<String>,  // "american", "european", "bermudan"
    pub expiration_date: Option<String>, // YYYY-MM-DD
    pub shares_per_contract: Option<u32>,
    pub strike_price: Option<f64>,
    pub underlying_ticker: Option<String>,
}

/// Underlying asset information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnderlyingAsset {
    pub aggregates: Option<Vec<Aggregate>>,
    pub change_to_break_even: Option<f64>,
    pub last_updated: Option<u64>, // Nanoseconds
    pub price: Option<f64>,
    pub ticker: Option<String>,
    pub timeframe: Option<String>,
    pub value: Option<f64>,
}

/// Aggregate/bar data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aggregate {
    pub c: Option<f64>,  // Close
    pub h: Option<f64>,  // High
    pub l: Option<f64>,  // Low
    pub n: Option<u64>,  // Number of transactions
    pub o: Option<f64>,  // Open
    pub t: Option<u64>,  // Timestamp (milliseconds)
    pub v: Option<f64>,  // Volume
    pub vw: Option<f64>, // Volume weighted average price
}

/// Day bar information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayBar {
    pub change: Option<f64>,
    pub change_percent: Option<f64>,
    pub close: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub open: Option<f64>,
    pub previous_close: Option<f64>,
    pub volume: Option<f64>,
    pub vwap: Option<f64>,
}

/// Contract metadata response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassiveContractMetadata {
    pub ticker: String,
    pub underlying_ticker: Option<String>,
    pub contract_type: Option<String>,
    pub exercise_style: Option<String>,
    pub expiration_date: Option<String>,
    pub strike_price: Option<f64>,
    pub shares_per_contract: Option<u32>,
    pub primary_exchange: Option<String>,
    pub cfi: Option<String>,
    pub additional_underlyings: Option<Vec<AdditionalUnderlying>>,
    pub correction: Option<i32>,
}

/// Additional underlying (for complex contracts)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditionalUnderlying {
    #[serde(rename = "type")]
    pub underlying_type: Option<String>,
    pub underlying: Option<String>,
    pub amount: Option<f64>,
}

/// Parse Massive API response
pub fn parse_response<T>(body: &str) -> MassiveResult<MassiveResponse<T>>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(body).map_err(|e| {
        MassiveError::Parse(format!("Failed to parse response: {}. Body: {}", e, body))
    })
}

/// Parse single result from response
pub fn parse_single_result<T>(body: &str) -> MassiveResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let response: MassiveResponse<T> = parse_response(body)?;

    response
        .results
        .ok_or_else(|| MassiveError::Parse("No results in response".to_string()))
}

/// Parse array results from response
pub fn parse_array_results<T>(body: &str) -> MassiveResult<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let response: MassiveResponse<Vec<T>> = parse_response(body)?;

    Ok(response.results.unwrap_or_default())
}

/// Extract error message from API response
pub fn extract_error_message(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(body)
        && let Some(status) = value.get("status").and_then(|v| v.as_str())
        && (status == "ERROR" || status == "error")
    {
        if let Some(message) = value.get("message").and_then(|v| v.as_str()) {
            return message.to_string();
        }
        if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
            return error.to_string();
        }
    }

    body.to_string()
}

/// Convert nanosecond timestamp to milliseconds
pub fn nanos_to_millis(nanos: u64) -> u64 {
    nanos / 1_000_000
}

/// Convert millisecond timestamp to nanoseconds
pub fn millis_to_nanos(millis: u64) -> u64 {
    millis * 1_000_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_snapshot_response() {
        let json = r#"{
            "status": "OK",
            "request_id": "test-123",
            "results": {
                "ticker": "O:AAPL240119C00150000",
                "name": "AAPL Jan 19 2024 $150 Call",
                "type": "options",
                "implied_volatility": 0.25,
                "open_interest": 1000,
                "greeks": {
                    "delta": 0.5,
                    "gamma": 0.05,
                    "theta": -0.02,
                    "vega": 0.15
                },
                "details": {
                    "contract_type": "call",
                    "exercise_style": "american",
                    "expiration_date": "2024-01-19",
                    "strike_price": 150.0,
                    "underlying_ticker": "AAPL"
                }
            }
        }"#;

        let result: Result<MassiveResponse<MassiveOptionSnapshot>, _> = parse_response(json);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.status, "OK");
        assert!(response.results.is_some());

        let snapshot = response.results.unwrap();
        assert_eq!(snapshot.ticker, "O:AAPL240119C00150000");
        assert_eq!(snapshot.implied_volatility, Some(0.25));
        assert_eq!(snapshot.open_interest, Some(1000));

        let greeks = snapshot.greeks.unwrap();
        assert_eq!(greeks.delta, Some(0.5));
        assert_eq!(greeks.gamma, Some(0.05));
    }

    #[test]
    fn test_parse_array_results() {
        let json = r#"{
            "status": "OK",
            "count": 2,
            "results": [
                {"ticker": "O:AAPL240119C00150000"},
                {"ticker": "O:AAPL240119P00150000"}
            ]
        }"#;

        let result: Result<Vec<MassiveOptionSnapshot>, _> = parse_array_results(json);
        assert!(result.is_ok());

        let snapshots = result.unwrap();
        assert_eq!(snapshots.len(), 2);
    }

    #[test]
    fn test_extract_error_message() {
        let error_json = r#"{
            "status": "ERROR",
            "message": "Symbol not found"
        }"#;

        let message = extract_error_message(error_json);
        assert_eq!(message, "Symbol not found");
    }

    #[test]
    fn test_nanos_to_millis() {
        assert_eq!(nanos_to_millis(1_000_000_000), 1000);
        assert_eq!(nanos_to_millis(5_500_000_000), 5500);
    }

    #[test]
    fn test_millis_to_nanos() {
        assert_eq!(millis_to_nanos(1000), 1_000_000_000);
        assert_eq!(millis_to_nanos(5500), 5_500_000_000);
    }
}
