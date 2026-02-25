//! Market data formatting utilities.

use data::{Price, Trade};

/// Format a price for display, rounding to the nearest tick.
///
/// Uses the tick size to determine the number of decimal places.
/// For example, ES with tick_size 0.25 shows 2 decimals;
/// ZN with tick_size 0.015625 shows 6 decimals.
pub fn format_price(
    price: Price,
    tick_size: Price,
) -> String {
    let tick_f = tick_size.to_f64();
    let decimals = if tick_f >= 1.0 {
        0
    } else if tick_f >= 0.1 {
        1
    } else if tick_f >= 0.01 {
        2
    } else if tick_f >= 0.001 {
        3
    } else if tick_f >= 0.0001 {
        4
    } else {
        6
    };

    let rounded =
        price.round_to_tick(tick_size);
    format!("{:.*}", decimals, rounded.to_f64())
}

/// Format a timestamp (millis since epoch) as a human-readable
/// UTC string.
pub fn format_timestamp(ts_ms: u64) -> String {
    let secs = (ts_ms / 1000) as i64;
    let nanos = ((ts_ms % 1000) * 1_000_000) as u32;
    if let Some(dt) =
        chrono::DateTime::from_timestamp(secs, nanos)
    {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        format!("{ts_ms}ms")
    }
}

/// Format volume in compact notation (1.2K, 3.4M, etc.).
pub fn format_volume(vol: u64) -> String {
    if vol >= 1_000_000 {
        format!("{:.1}M", vol as f64 / 1_000_000.0)
    } else if vol >= 1_000 {
        format!("{:.1}K", vol as f64 / 1_000.0)
    } else {
        vol.to_string()
    }
}

/// Format a single trade as a compact string.
pub fn format_trade(
    trade: &Trade,
    tick_size: Price,
) -> String {
    let ts = format_timestamp(trade.time.to_millis());
    let price =
        format_price(trade.price, tick_size);
    let side = if trade.is_buy() { "BUY" } else { "SELL" };
    let qty = trade.quantity.value();
    format!("{ts} {side} {qty:.0}@{price}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::Price;

    #[test]
    fn test_format_price_es() {
        let price = Price::from_f64(5025.75);
        let tick = Price::from_f64(0.25);
        let formatted = format_price(price, tick);
        assert_eq!(formatted, "5025.75");
    }

    #[test]
    fn test_format_price_gc() {
        let price = Price::from_f64(2045.30);
        let tick = Price::from_f64(0.10);
        let formatted = format_price(price, tick);
        assert_eq!(formatted, "2045.3");
    }

    #[test]
    fn test_format_volume_compact() {
        assert_eq!(format_volume(500), "500");
        assert_eq!(format_volume(1500), "1.5K");
        assert_eq!(format_volume(2_500_000), "2.5M");
    }

    #[test]
    fn test_format_timestamp() {
        // 2025-01-15 12:30:00 UTC
        let ts = 1736944200000u64;
        let formatted = format_timestamp(ts);
        assert!(
            formatted.contains("2025-01-15"),
            "Expected date, got: {formatted}"
        );
    }
}
