//! Type conversion — Databento types → domain types
//!
//! Key conversion: Databento price (10^-9) → domain Price (10^-8)

use crate::domain::{Depth, Price, Quantity, Side, Timestamp, Trade};
use databento::dbn::{Mbp10Msg, SType, TradeMsg};
use time::OffsetDateTime;

use super::DatabentoError;

// ── Price conversion ─────────────────────────────────────────────────────

/// Convert Databento price (10^-9 precision) to domain Price (10^-8 precision)
pub fn convert_databento_price(databento_price: i64) -> Price {
    Price::from_units((databento_price + databento_price.signum() * 5) / 10)
}

// ── Time conversions ─────────────────────────────────────────────────────

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

// ── Symbol type ──────────────────────────────────────────────────────────

pub fn determine_stype(symbol: &str) -> SType {
    if symbol.contains(".c.") {
        SType::Continuous
    } else if symbol.ends_with(".FUT") || symbol.ends_with(".OPT") {
        SType::Parent
    } else {
        SType::RawSymbol
    }
}

// ── Domain type conversion ───────────────────────────────────────────────

/// Convert a Databento `TradeMsg` to a domain `Trade` (produced at boundary)
pub fn trade_msg_to_domain(msg: &TradeMsg) -> Result<Trade, DatabentoError> {
    let ts = msg
        .ts_recv()
        .ok_or_else(|| DatabentoError::Config("missing ts_recv".to_string()))?;
    let time_ms = (ts.unix_timestamp_nanos() / 1_000_000) as u64;

    let dbn_side = msg.side()?;
    let side = match dbn_side {
        databento::dbn::Side::Ask => Side::Sell,
        _ => Side::Buy,
    };

    Ok(Trade::new(
        Timestamp::from_millis(time_ms),
        convert_databento_price(msg.price),
        Quantity(msg.size as f64),
        side,
    ))
}

/// Convert a Databento `Mbp10Msg` to a domain `Depth` snapshot
pub fn mbp10_to_domain(msg: &Mbp10Msg) -> Result<Depth, DatabentoError> {
    let ts = msg
        .ts_recv()
        .ok_or_else(|| DatabentoError::Config("missing ts_recv".to_string()))?;
    let time_ms = (ts.unix_timestamp_nanos() / 1_000_000) as u64;

    let mut depth = Depth::new(time_ms);

    for level in &msg.levels {
        if level.bid_px != databento::dbn::UNDEF_PRICE && level.bid_sz > 0 {
            depth.bids.insert(
                convert_databento_price(level.bid_px).units(),
                level.bid_sz as f32,
            );
        }
        if level.ask_px != databento::dbn::UNDEF_PRICE && level.ask_sz > 0 {
            depth.asks.insert(
                convert_databento_price(level.ask_px).units(),
                level.ask_sz as f32,
            );
        }
    }

    Ok(depth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_conversion_rounds_not_truncates() {
        let price = convert_databento_price(1_234_567_895);
        assert_eq!(price.units(), 123_456_790);
    }

    #[test]
    fn test_price_conversion_rounds_down_below_half() {
        let price = convert_databento_price(1_234_567_894);
        assert_eq!(price.units(), 123_456_789);
    }

    #[test]
    fn test_price_conversion_negative_rounds_correctly() {
        let price = convert_databento_price(-1_234_567_895);
        assert_eq!(price.units(), -123_456_790);
    }

    #[test]
    fn test_price_conversion_zero() {
        let price = convert_databento_price(0);
        assert_eq!(price.units(), 0);
    }

    #[test]
    fn test_price_conversion_exact() {
        let price = convert_databento_price(1_234_567_890);
        assert_eq!(price.units(), 123_456_789);
    }

    #[test]
    fn test_determine_stype() {
        assert!(matches!(determine_stype("ES.c.0"), SType::Continuous));
        assert!(matches!(determine_stype("ES.FUT"), SType::Parent));
        assert!(matches!(determine_stype("ESH4"), SType::RawSymbol));
    }
}
