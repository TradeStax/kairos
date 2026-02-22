//! Type conversion and mapping between Databento types and domain types
//!
//! Handles:
//! - Price conversions (Databento fixed-point → domain Price type)
//! - Time conversions (chrono ↔ time crate)
//! - Symbol type determination
//! - Ticker info and symbology

use super::DatabentoError;
use crate::{FuturesTicker, FuturesTickerInfo, FuturesVenue, TickerStats, util::Price};
use databento::dbn::{OhlcvMsg, SType, Schema, SymbolIndex};
use databento::{HistoricalClient, historical::timeseries::GetRangeParams};
use std::collections::HashMap;
use time::OffsetDateTime;

// ============================================================================
// PRICE CONVERSIONS
// ============================================================================

/// Convert databento price (10^-9 precision) to domain Price type
pub fn convert_databento_price(databento_price: i64) -> Price {
    // Databento uses nanosecond precision (10^-9)
    // Our Price type uses 10^-8 precision
    // So divide by 10 to convert
    Price::from_units(databento_price / 10)
}

// ============================================================================
// TIME CONVERSIONS
// ============================================================================

/// Convert chrono DateTime to time OffsetDateTime
pub fn chrono_to_time(dt: chrono::DateTime<chrono::Utc>) -> Result<OffsetDateTime, DatabentoError> {
    let unix_ts = dt.timestamp();
    let nanos = dt.timestamp_subsec_nanos();

    OffsetDateTime::from_unix_timestamp(unix_ts)
        .map(|odt| {
            if nanos > 0 {
                odt + time::Duration::nanoseconds(nanos as i64)
            } else {
                odt
            }
        })
        .map_err(|e| DatabentoError::Config(format!("Invalid timestamp: {}", e)))
}

// ============================================================================
// SYMBOL TYPE DETERMINATION
// ============================================================================

/// Determine symbol type from symbol string
pub fn determine_stype(symbol: &str) -> SType {
    if symbol.contains(".c.") {
        SType::Continuous
    } else if symbol.ends_with(".FUT") || symbol.ends_with(".OPT") {
        SType::Parent
    } else {
        SType::RawSymbol
    }
}

// ============================================================================
// SYMBOLOGY - Ticker Info for Continuous Contracts
// ============================================================================

/// Get ticker info for continuous contracts (instant - no API call)
///
/// Returns standardized futures contracts with proper tick sizes and contract multipliers
pub fn get_continuous_ticker_info() -> HashMap<FuturesTicker, Option<FuturesTickerInfo>> {
    let venue = FuturesVenue::CMEGlobex;

    // Product specs: (symbol, tick_size, min_qty, contract_size)
    let products = vec![
        // Equity Indices
        ("ES.c.0", 0.25, 1.0, 50.0), // E-mini S&P 500
        ("NQ.c.0", 0.25, 1.0, 20.0), // E-mini Nasdaq-100
        ("YM.c.0", 1.0, 1.0, 5.0),   // E-mini Dow ($5)
        ("RTY.c.0", 0.1, 1.0, 50.0), // E-mini Russell 2000
        // Treasuries
        ("ZN.c.0", 0.015625, 1.0, 1000.0), // 10-Year T-Note (1/64th)
        ("ZB.c.0", 0.03125, 1.0, 1000.0),  // 30-Year T-Bond (1/32nd)
        ("ZT.c.0", 0.0078125, 1.0, 2000.0), // 2-Year T-Note (1/128th)
        ("ZF.c.0", 0.0078125, 1.0, 1000.0), // 5-Year T-Note (1/128th)
        // Metals
        ("GC.c.0", 0.10, 1.0, 100.0),   // Gold ($100/oz)
        ("SI.c.0", 0.005, 1.0, 5000.0), // Silver ($5000/contract)
        // Energy
        ("CL.c.0", 0.01, 1.0, 1000.0), // Crude Oil WTI ($1000/contract)
        ("NG.c.0", 0.001, 1.0, 10000.0), // Natural Gas ($10,000/contract)
    ];

    let mut result = HashMap::new();

    for (symbol, tick_size, min_qty, contract_size) in products {
        let ticker = FuturesTicker::new(symbol, venue);
        let ticker_info = FuturesTickerInfo::new(ticker, tick_size, min_qty, contract_size);
        result.insert(ticker, Some(ticker_info));
    }

    log::debug!("Loaded {} continuous futures ticker info", result.len());
    result
}

/// Fetch REAL historical prices from databento in ONE API call (FAST)
/// NOTE: Databento historical API has ~1 day delay. This fetches YESTERDAY's closing prices.
pub async fn fetch_historical_prices(
    config: super::DatabentoConfig,
    as_of_date: Option<chrono::NaiveDate>,
) -> Result<HashMap<FuturesTicker, TickerStats>, DatabentoError> {
    // Determine which date to fetch
    // Databento historical data has ~12-24 hour delay
    // Use data from 2 days ago to be safe (definitely available)
    let target_date = as_of_date.unwrap_or_else(|| {
        let two_days_ago = chrono::Utc::now().date_naive() - chrono::Duration::days(2);
        log::debug!(
            "Fetching data from {} (2 days ago - guaranteed available)",
            two_days_ago
        );
        two_days_ago
    });

    log::debug!(
        "Fetching historical prices for {} from databento (ONE batch query)",
        target_date
    );

    let mut client = HistoricalClient::builder()
        .key(config.api_key.clone())?
        .build()?;

    let symbols = vec![
        "ES.c.0", "NQ.c.0", "YM.c.0", "RTY.c.0", "ZN.c.0", "ZB.c.0", "ZT.c.0", "ZF.c.0", "GC.c.0",
        "SI.c.0", "CL.c.0", "NG.c.0",
    ];

    // Convert date
    let datetime = target_date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| DatabentoError::Config("Failed to create datetime".to_string()))?;
    let timestamp = datetime.and_utc().timestamp();
    let offset_dt = time::OffsetDateTime::from_unix_timestamp(timestamp)
        .map_err(|e| DatabentoError::Config(format!("Invalid timestamp: {}", e)))?;
    let time_date = offset_dt.date();

    log::debug!(
        "Querying databento for ALL {} symbols in one request for {}",
        symbols.len(),
        time_date
    );

    // ONE API call for all symbols
    let params = GetRangeParams::builder()
        .dataset(config.dataset)
        .schema(Schema::Ohlcv1D)
        .symbols(symbols.clone())
        .stype_in(SType::Continuous)
        .date_time_range(time_date)
        .build();

    let mut decoder = client.timeseries().get_range(&params).await?;
    let symbol_map = decoder.metadata().symbol_map()?;

    let mut all_stats = HashMap::new();
    let venue = FuturesVenue::CMEGlobex;

    // Decode all bars
    while let Some(bar) = decoder.decode_record::<OhlcvMsg>().await? {
        let symbol = symbol_map.get_for_rec(bar).ok_or_else(|| {
            DatabentoError::Config(format!(
                "No symbol mapping for instrument {}",
                bar.hd.instrument_id
            ))
        })?;

        // Convert prices (databento uses 10^-9, we use 10^-8)
        let close_price = convert_databento_price(bar.close).to_f64();
        let open_price = convert_databento_price(bar.open).to_f64();
        let daily_change_pct = if open_price > 0.0 {
            ((close_price - open_price) / open_price * 100.0) as f32
        } else {
            0.0
        };

        let stats = TickerStats {
            mark_price: close_price as f32,
            daily_price_chg: daily_change_pct,
            daily_volume: bar.volume as f32,
        };

        let ticker = FuturesTicker::new(symbol, venue);
        all_stats.insert(ticker, stats);

        log::debug!(
            "{}: close={:.2}, change={:+.2}%, volume={:.0}",
            symbol,
            close_price,
            daily_change_pct,
            bar.volume
        );
    }

    if all_stats.is_empty() {
        return Err(DatabentoError::SymbolNotFound(format!(
            "No historical data found for {}. Market may have been closed.",
            target_date
        )));
    }

    log::info!(
        "Successfully fetched {} prices from databento for {}",
        all_stats.len(),
        target_date
    );
    Ok(all_stats)
}
