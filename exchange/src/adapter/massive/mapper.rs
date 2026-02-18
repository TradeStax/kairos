use super::decoder::*;
use super::{MassiveError, MassiveResult};
use flowsurface_data::domain::{
    ExerciseStyle, Greek, OptionChain, OptionContract, OptionSnapshot, OptionType, Price,
    Quantity, Timestamp,
};
use chrono::NaiveDate;

/// Convert Massive snapshot to domain OptionSnapshot
pub fn convert_snapshot_response(
    massive_snapshot: MassiveOptionSnapshot,
) -> MassiveResult<OptionSnapshot> {
    // Extract contract details
    let details = massive_snapshot
        .details
        .ok_or_else(|| MassiveError::InvalidData("Missing contract details".to_string()))?;

    let contract = convert_contract_details(&massive_snapshot.ticker, details)?;

    // Create base snapshot
    let timestamp = massive_snapshot
        .last_updated
        .map(nanos_to_millis)
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis() as u64);

    let mut snapshot = OptionSnapshot::new(contract, Timestamp(timestamp));

    // Fill in market data
    if let Some(last_trade) = massive_snapshot.last_trade {
        snapshot.last_price = last_trade.price.map(Price::from_f64);
    }

    if let Some(last_quote) = massive_snapshot.last_quote {
        snapshot.bid = last_quote.bid.map(Price::from_f64);
        snapshot.ask = last_quote.ask.map(Price::from_f64);
        snapshot.bid_size = last_quote.bid_size.map(Quantity);
        snapshot.ask_size = last_quote.ask_size.map(Quantity);
    }

    // Fill in Greeks
    if let Some(greeks) = massive_snapshot.greeks {
        snapshot.greeks = Greek {
            delta: greeks.delta,
            gamma: greeks.gamma,
            theta: greeks.theta,
            vega: greeks.vega,
            rho: None,
        };
    }

    // Fill in IV and OI
    snapshot.implied_volatility = massive_snapshot.implied_volatility;
    snapshot.open_interest = massive_snapshot.open_interest;
    snapshot.break_even_price = massive_snapshot
        .break_even_price
        .map(Price::from_f64);

    // Fill in underlying price
    if let Some(underlying) = massive_snapshot.underlying_asset {
        snapshot.underlying_price = underlying.price.map(Price::from_f64);
    }

    // Fill in volume from day bar
    if let Some(day) = massive_snapshot.day {
        snapshot.volume = day.volume.map(|v| v as u64);
    }

    Ok(snapshot)
}

/// Convert Massive contract metadata to domain OptionContract
pub fn convert_contract_response(
    massive_contract: MassiveContractMetadata,
) -> MassiveResult<OptionContract> {
    let underlying_ticker = massive_contract
        .underlying_ticker
        .ok_or_else(|| MassiveError::InvalidData("Missing underlying ticker".to_string()))?;

    let strike_price = massive_contract
        .strike_price
        .ok_or_else(|| MassiveError::InvalidData("Missing strike price".to_string()))?;

    let expiration_str = massive_contract
        .expiration_date
        .ok_or_else(|| MassiveError::InvalidData("Missing expiration date".to_string()))?;

    let expiration_date = NaiveDate::parse_from_str(&expiration_str, "%Y-%m-%d")
        .map_err(|e| MassiveError::DateTime(format!("Invalid expiration date: {}", e)))?;

    let contract_type = parse_contract_type(
        massive_contract
            .contract_type
            .as_deref()
            .unwrap_or("other"),
    );

    let exercise_style = parse_exercise_style(
        massive_contract
            .exercise_style
            .as_deref()
            .unwrap_or("american"),
    );

    let mut contract = OptionContract::new(
        massive_contract.ticker,
        underlying_ticker,
        Price::from_f64(strike_price),
        expiration_date,
        contract_type,
        exercise_style,
    );

    // Fill in optional fields
    if let Some(shares) = massive_contract.shares_per_contract {
        contract.shares_per_contract = shares;
    } else {
        log::warn!(
            "Contract {} missing shares_per_contract from API, \
             using default of 100",
            contract.ticker
        );
    }

    contract.primary_exchange = massive_contract.primary_exchange;
    contract.cfi = massive_contract.cfi;

    Ok(contract)
}

/// Convert array of Massive snapshots to OptionChain
pub fn convert_chain_response(
    underlying_ticker: String,
    date: NaiveDate,
    massive_snapshots: Vec<MassiveOptionSnapshot>,
) -> MassiveResult<OptionChain> {
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;
    let mut chain = OptionChain::new(underlying_ticker, date, Timestamp(timestamp));

    // Get underlying price from first snapshot
    if let Some(first) = massive_snapshots.first()
        && let Some(ref underlying) = first.underlying_asset {
            chain.underlying_price = underlying.price.map(Price::from_f64);
        }

    // Convert each snapshot
    for massive_snapshot in massive_snapshots {
        match convert_snapshot_response(massive_snapshot) {
            Ok(snapshot) => chain.add_contract(snapshot),
            Err(e) => {
                log::warn!("Failed to convert snapshot: {}", e);
                // Continue with other snapshots
            }
        }
    }

    Ok(chain)
}

/// Convert contract details from snapshot to OptionContract
fn convert_contract_details(
    ticker: &str,
    details: ContractDetails,
) -> MassiveResult<OptionContract> {
    let underlying_ticker = details
        .underlying_ticker
        .ok_or_else(|| MassiveError::InvalidData("Missing underlying ticker".to_string()))?;

    let strike_price = details
        .strike_price
        .ok_or_else(|| MassiveError::InvalidData("Missing strike price".to_string()))?;

    let expiration_str = details
        .expiration_date
        .ok_or_else(|| MassiveError::InvalidData("Missing expiration date".to_string()))?;

    let expiration_date = NaiveDate::parse_from_str(&expiration_str, "%Y-%m-%d")
        .map_err(|e| MassiveError::DateTime(format!("Invalid expiration date: {}", e)))?;

    let contract_type = parse_contract_type(details.contract_type.as_deref().unwrap_or("other"));

    let exercise_style =
        parse_exercise_style(details.exercise_style.as_deref().unwrap_or("american"));

    let mut contract = OptionContract::new(
        ticker.to_string(),
        underlying_ticker,
        Price::from_f64(strike_price),
        expiration_date,
        contract_type,
        exercise_style,
    );

    if let Some(shares) = details.shares_per_contract {
        contract.shares_per_contract = shares;
    } else {
        log::warn!(
            "Contract {} missing shares_per_contract from API, \
             using default of 100",
            ticker
        );
    }

    Ok(contract)
}

/// Parse contract type string to enum
fn parse_contract_type(type_str: &str) -> OptionType {
    match type_str.to_lowercase().as_str() {
        "call" => OptionType::Call,
        "put" => OptionType::Put,
        _ => OptionType::Other,
    }
}

/// Parse exercise style string to enum
fn parse_exercise_style(style_str: &str) -> ExerciseStyle {
    match style_str.to_lowercase().as_str() {
        "american" => ExerciseStyle::American,
        "european" => ExerciseStyle::European,
        "bermudan" => ExerciseStyle::Bermudan,
        _ => ExerciseStyle::American, // Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_snapshot() -> MassiveOptionSnapshot {
        MassiveOptionSnapshot {
            ticker: "O:AAPL240119C00150000".to_string(),
            name: Some("AAPL Jan 19 2024 $150 Call".to_string()),
            asset_type: Some("options".to_string()),
            market_status: Some("open".to_string()),
            timeframe: Some("REAL-TIME".to_string()),
            last_updated: Some(1705680000000000000), // Nanoseconds
            last_trade: Some(LastTrade {
                conditions: None,
                exchange: None,
                price: Some(5.50),
                sip_timestamp: None,
                size: Some(10.0),
            }),
            last_quote: Some(LastQuote {
                ask: Some(5.55),
                ask_size: Some(100.0),
                bid: Some(5.45),
                bid_size: Some(150.0),
                last_updated: None,
                timeframe: None,
            }),
            greeks: Some(Greeks {
                delta: Some(0.5),
                gamma: Some(0.05),
                theta: Some(-0.02),
                vega: Some(0.15),
            }),
            implied_volatility: Some(0.25),
            open_interest: Some(1000),
            break_even_price: Some(155.50),
            details: Some(ContractDetails {
                contract_type: Some("call".to_string()),
                exercise_style: Some("american".to_string()),
                expiration_date: Some("2024-01-19".to_string()),
                shares_per_contract: Some(100),
                strike_price: Some(150.0),
                underlying_ticker: Some("AAPL".to_string()),
            }),
            underlying_asset: Some(UnderlyingAsset {
                aggregates: None,
                change_to_break_even: None,
                last_updated: None,
                price: Some(152.50),
                ticker: Some("AAPL".to_string()),
                timeframe: None,
                value: None,
            }),
            day: Some(DayBar {
                change: Some(0.50),
                change_percent: Some(10.0),
                close: None,
                high: None,
                low: None,
                open: None,
                previous_close: None,
                volume: Some(5000.0),
                vwap: None,
            }),
        }
    }

    #[test]
    fn test_convert_snapshot_response() {
        let massive_snapshot = create_test_snapshot();
        let result = convert_snapshot_response(massive_snapshot);

        assert!(result.is_ok());

        let snapshot = result.unwrap();
        assert_eq!(snapshot.contract.ticker, "O:AAPL240119C00150000");
        assert_eq!(snapshot.contract.underlying_ticker, "AAPL");
        assert_eq!(snapshot.contract.strike_price, Price::from_f64(150.0));
        assert_eq!(snapshot.contract.contract_type, OptionType::Call);
        assert_eq!(snapshot.contract.exercise_style, ExerciseStyle::American);

        assert_eq!(snapshot.last_price, Some(Price::from_f64(5.50)));
        assert_eq!(snapshot.bid, Some(Price::from_f64(5.45)));
        assert_eq!(snapshot.ask, Some(Price::from_f64(5.55)));
        assert_eq!(snapshot.implied_volatility, Some(0.25));
        assert_eq!(snapshot.open_interest, Some(1000));
        assert_eq!(snapshot.underlying_price, Some(Price::from_f64(152.50)));

        assert_eq!(snapshot.greeks.delta, Some(0.5));
        assert_eq!(snapshot.greeks.gamma, Some(0.05));
    }

    #[test]
    fn test_convert_contract_response() {
        let massive_contract = MassiveContractMetadata {
            ticker: "O:AAPL240119C00150000".to_string(),
            underlying_ticker: Some("AAPL".to_string()),
            contract_type: Some("call".to_string()),
            exercise_style: Some("american".to_string()),
            expiration_date: Some("2024-01-19".to_string()),
            strike_price: Some(150.0),
            shares_per_contract: Some(100),
            primary_exchange: Some("CBOE".to_string()),
            cfi: Some("OCAXXX".to_string()),
            additional_underlyings: None,
            correction: None,
        };

        let result = convert_contract_response(massive_contract);
        assert!(result.is_ok());

        let contract = result.unwrap();
        assert_eq!(contract.ticker, "O:AAPL240119C00150000");
        assert_eq!(contract.underlying_ticker, "AAPL");
        assert_eq!(contract.strike_price, Price::from_f64(150.0));
        assert_eq!(contract.contract_type, OptionType::Call);
        assert_eq!(contract.shares_per_contract, 100);
    }

    #[test]
    fn test_convert_chain_response() {
        let massive_snapshots = vec![
            create_test_snapshot(),
            {
                let mut put_snapshot = create_test_snapshot();
                put_snapshot.ticker = "O:AAPL240119P00150000".to_string();
                put_snapshot.details = Some(ContractDetails {
                    contract_type: Some("put".to_string()),
                    exercise_style: Some("american".to_string()),
                    expiration_date: Some("2024-01-19".to_string()),
                    shares_per_contract: Some(100),
                    strike_price: Some(150.0),
                    underlying_ticker: Some("AAPL".to_string()),
                });
                put_snapshot
            },
        ];

        let date = NaiveDate::from_ymd_opt(2024, 1, 19).unwrap();
        let result = convert_chain_response("AAPL".to_string(), date, massive_snapshots);

        assert!(result.is_ok());

        let chain = result.unwrap();
        assert_eq!(chain.underlying_ticker, "AAPL");
        assert_eq!(chain.date, date);
        assert_eq!(chain.contract_count(), 2);
        assert_eq!(chain.calls().len(), 1);
        assert_eq!(chain.puts().len(), 1);
    }

    #[test]
    fn test_parse_contract_type() {
        assert_eq!(parse_contract_type("call"), OptionType::Call);
        assert_eq!(parse_contract_type("Call"), OptionType::Call);
        assert_eq!(parse_contract_type("put"), OptionType::Put);
        assert_eq!(parse_contract_type("PUT"), OptionType::Put);
        assert_eq!(parse_contract_type("unknown"), OptionType::Other);
    }

    #[test]
    fn test_parse_exercise_style() {
        assert_eq!(parse_exercise_style("american"), ExerciseStyle::American);
        assert_eq!(parse_exercise_style("AMERICAN"), ExerciseStyle::American);
        assert_eq!(parse_exercise_style("european"), ExerciseStyle::European);
        assert_eq!(parse_exercise_style("bermudan"), ExerciseStyle::Bermudan);
        assert_eq!(parse_exercise_style("unknown"), ExerciseStyle::American);
    }
}
