//! Symbology — ticker info and historical price fetching

use crate::domain::{FuturesTicker, FuturesTickerInfo, FuturesVenue, TickerStats};
use databento::{
    HistoricalClient,
    dbn::{OhlcvMsg, SType, Schema, SymbolIndex},
    historical::timeseries::GetRangeParams,
};
use std::collections::HashMap;
use time::OffsetDateTime;

use super::mapper::convert_databento_price;
use super::{DatabentoConfig, DatabentoError};

/// Get ticker info for all supported continuous contracts (no API call)
pub fn get_continuous_ticker_info() -> HashMap<FuturesTicker, Option<FuturesTickerInfo>> {
    let venue = FuturesVenue::CMEGlobex;

    let products = vec![
        ("ES.c.0", 0.25f32, 1.0f32, 50.0f32),
        ("NQ.c.0", 0.25, 1.0, 20.0),
        ("YM.c.0", 1.0, 1.0, 5.0),
        ("RTY.c.0", 0.1, 1.0, 50.0),
        ("ZN.c.0", 0.015625, 1.0, 1000.0),
        ("ZB.c.0", 0.03125, 1.0, 1000.0),
        ("ZT.c.0", 0.0078125, 1.0, 2000.0),
        ("ZF.c.0", 0.0078125, 1.0, 1000.0),
        ("GC.c.0", 0.10, 1.0, 100.0),
        ("SI.c.0", 0.005, 1.0, 5000.0),
        ("CL.c.0", 0.01, 1.0, 1000.0),
        ("NG.c.0", 0.001, 1.0, 10000.0),
    ];

    let mut result = HashMap::new();
    for (symbol, tick_size, min_qty, contract_size) in products {
        let ticker = FuturesTicker::new(symbol, venue);
        let info = FuturesTickerInfo::new(ticker, tick_size, min_qty, contract_size);
        result.insert(ticker, Some(info));
    }

    log::debug!("Loaded {} continuous futures ticker info", result.len());
    result
}

/// Fetch historical prices for all supported symbols in one API call
pub async fn fetch_historical_prices(
    config: DatabentoConfig,
    as_of_date: Option<chrono::NaiveDate>,
) -> Result<HashMap<FuturesTicker, TickerStats>, DatabentoError> {
    let target_date =
        as_of_date.unwrap_or_else(|| chrono::Utc::now().date_naive() - chrono::Duration::days(2));

    log::debug!(
        "Fetching historical prices for {} from databento",
        target_date
    );

    let mut client = HistoricalClient::builder()
        .key(config.api_key.clone())?
        .build()?;

    let symbols = vec![
        "ES.c.0", "NQ.c.0", "YM.c.0", "RTY.c.0", "ZN.c.0", "ZB.c.0", "ZT.c.0", "ZF.c.0", "GC.c.0",
        "SI.c.0", "CL.c.0", "NG.c.0",
    ];

    let datetime = target_date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| DatabentoError::Config("Failed to create datetime".to_string()))?;
    let timestamp = datetime.and_utc().timestamp();
    let offset_dt = OffsetDateTime::from_unix_timestamp(timestamp)
        .map_err(|e| DatabentoError::Config(format!("Invalid timestamp: {}", e)))?;
    let time_date = offset_dt.date();

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

    while let Some(bar) = decoder.decode_record::<OhlcvMsg>().await? {
        let symbol = symbol_map.get_for_rec(bar).ok_or_else(|| {
            DatabentoError::Config(format!(
                "No symbol mapping for instrument {}",
                bar.hd.instrument_id
            ))
        })?;

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
    }

    if all_stats.is_empty() {
        return Err(DatabentoError::SymbolNotFound(format!(
            "No historical data found for {}. Market may have been closed.",
            target_date
        )));
    }

    log::info!(
        "Fetched {} prices from databento for {}",
        all_stats.len(),
        target_date
    );
    Ok(all_stats)
}
