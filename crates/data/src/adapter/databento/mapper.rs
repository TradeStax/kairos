//! Type conversion from Databento wire types to domain types.
//!
//! The key conversion is price precision: Databento uses 10^-9 fixed-point
//! while the domain [`Price`] type uses 10^-8. Values are rounded (not truncated)
//! during conversion.

use databento::dbn::{Mbp10Msg, SType, TradeMsg};

use crate::domain::{Depth, Price, Quantity, Side, Timestamp, Trade};

use super::DatabentoError;

// ── Price conversion ─────────────────────────────────────────────────────

/// Converts a Databento price (10^-9 precision) to a domain [`Price`] (10^-8 precision)
///
/// Uses banker-style rounding: adds half of the divisor (5) before dividing by 10,
/// with sign-aware adjustment for negative prices.
#[must_use]
pub(crate) fn convert_databento_price(databento_price: i64) -> Price {
    Price::from_units((databento_price + databento_price.signum() * 5) / 10)
}

// ── Symbol type ──────────────────────────────────────────────────────────

/// Determines the Databento [`SType`] based on symbol naming conventions
///
/// - `"ES.c.0"` → [`SType::Continuous`]
/// - `"ES.FUT"` / `"ES.OPT"` → [`SType::Parent`]
/// - `"ESH4"` → [`SType::RawSymbol`]
#[must_use]
pub(crate) fn determine_stype(symbol: &str) -> SType {
    if symbol.contains(".c.") {
        SType::Continuous
    } else if symbol.ends_with(".FUT") || symbol.ends_with(".OPT") {
        SType::Parent
    } else {
        SType::RawSymbol
    }
}

// ── Domain type conversion ───────────────────────────────────────────────

/// Converts a Databento [`TradeMsg`] to a domain [`Trade`]
pub(crate) fn trade_msg_to_domain(msg: &TradeMsg) -> Result<Trade, DatabentoError> {
    let ts = msg
        .ts_recv()
        .ok_or_else(|| DatabentoError::Config("missing ts_recv".to_string()))?;
    let time_ms = (ts.unix_timestamp_nanos() / 1_000_000) as u64;

    let dbn_side = msg.side()?;
    let side = match dbn_side {
        databento::dbn::Side::Ask => Side::Sell,
        databento::dbn::Side::Bid => Side::Buy,
        databento::dbn::Side::None => {
            log::debug!("Trade with no side specified, defaulting to Buy");
            Side::Buy
        }
    };

    Ok(Trade::new(
        Timestamp::from_millis(time_ms),
        convert_databento_price(msg.price),
        Quantity(msg.size as f64),
        side,
    ))
}

/// Converts a Databento [`Mbp10Msg`] to a domain [`Depth`] snapshot
pub(crate) fn mbp10_to_domain(msg: &Mbp10Msg) -> Result<Depth, DatabentoError> {
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
