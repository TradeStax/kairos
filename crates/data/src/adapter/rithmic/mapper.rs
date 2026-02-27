//! Rithmic-to-domain type mapping.
//!
//! Converts Rithmic protobuf wire messages into domain types ([`Trade`],
//! [`Depth`]). All output uses domain types directly -- no intermediate
//! exchange types. Handles timestamp conversion (ssboe + usecs to millis)
//! and aggressor side mapping.

use super::protocol::{RithmicMessage, RithmicResponse};
use crate::domain::{Depth, Price, Quantity, Side, Timestamp, Trade};

/// Converts a Rithmic `LastTrade` message to a domain [`Trade`].
///
/// Returns `None` if the message is not a `LastTrade` variant or if
/// required fields (timestamp, price, size) are missing.
pub fn map_last_trade(msg: &RithmicMessage) -> Option<Trade> {
    match msg {
        RithmicMessage::LastTrade(lt) => {
            let ssboe = lt.ssboe? as u64;
            let usecs = lt.usecs.unwrap_or(0) as u64;
            // Round microseconds to nearest millisecond
            let time_ms = ssboe * 1000 + (usecs + 500) / 1000;

            let price = lt.trade_price?;
            let size = lt.trade_size? as f64;

            // aggressor: 1 = Buy, 2 = Sell (Rithmic protobuf convention)
            let side = match lt.aggressor {
                Some(1) => Side::Buy,
                Some(2) => Side::Sell,
                other => {
                    log::debug!(
                        "Unknown Rithmic aggressor value: {:?}, defaulting to Sell",
                        other
                    );
                    Side::Sell
                }
            };

            Some(Trade {
                time: Timestamp::from_millis(time_ms),
                price: Price::from_f64(price),
                quantity: Quantity(size),
                side,
            })
        }
        _ => None,
    }
}

/// Converts a Rithmic `BestBidOffer` message to a domain [`Depth`].
///
/// Produces a single-level depth snapshot from the top-of-book quote.
pub fn map_bbo_to_depth(msg: &RithmicMessage) -> Option<Depth> {
    match msg {
        RithmicMessage::BestBidOffer(bbo) => {
            let ssboe = bbo.ssboe? as u64;
            let usecs = bbo.usecs.unwrap_or(0) as u64;
            let time_ms = ssboe * 1000 + (usecs + 500) / 1000;

            let mut depth = Depth::new(time_ms);

            if let (Some(bid_price), Some(bid_size)) = (bbo.bid_price, bbo.bid_size) {
                let price_units = Price::from_f64(bid_price).units();
                depth.bids.insert(price_units, bid_size as f32);
            }

            if let (Some(ask_price), Some(ask_size)) = (bbo.ask_price, bbo.ask_size) {
                let price_units = Price::from_f64(ask_price).units();
                depth.asks.insert(price_units, ask_size as f32);
            }

            Some(depth)
        }
        _ => None,
    }
}

/// Converts a Rithmic `OrderBook` message to a domain [`Depth`].
///
/// Returns `None` if price/size arrays are mismatched or both sides
/// are empty.
pub fn map_orderbook_to_depth(msg: &RithmicMessage) -> Option<Depth> {
    match msg {
        RithmicMessage::OrderBook(ob) => {
            let ssboe = ob.ssboe? as u64;
            let usecs = ob.usecs.unwrap_or(0) as u64;
            let time_ms = ssboe * 1000 + (usecs + 500) / 1000;

            let mut depth = Depth::new(time_ms);

            if ob.bid_price.len() != ob.bid_size.len() {
                log::warn!(
                    "OrderBook bid price/size mismatch: {} vs {} — rejecting",
                    ob.bid_price.len(),
                    ob.bid_size.len()
                );
                return None;
            }
            if ob.ask_price.len() != ob.ask_size.len() {
                log::warn!(
                    "OrderBook ask price/size mismatch: {} vs {} — rejecting",
                    ob.ask_price.len(),
                    ob.ask_size.len()
                );
                return None;
            }

            for (price, size) in ob.bid_price.iter().zip(ob.bid_size.iter()) {
                depth
                    .bids
                    .insert(Price::from_f64(*price).units(), *size as f32);
            }
            for (price, size) in ob.ask_price.iter().zip(ob.ask_size.iter()) {
                depth
                    .asks
                    .insert(Price::from_f64(*price).units(), *size as f32);
            }

            if depth.bids.is_empty() && depth.asks.is_empty() {
                return None;
            }

            Some(depth)
        }
        _ => None,
    }
}

/// Converts historical tick bar replay responses to domain [`Trade`]s.
///
/// Handles both `ResponseTickBarReplay` bars and `LastTrade` messages
/// that may appear in the response stream. Infers trade side from
/// bid/ask volume split.
pub fn map_tick_replay_to_trades(responses: &[RithmicResponse]) -> Vec<Trade> {
    let mut trades = Vec::with_capacity(responses.len());
    trades.extend(responses.iter().filter_map(|r| match &r.message {
        RithmicMessage::ResponseTickBarReplay(bar) => {
            let ssboe = bar.data_bar_ssboe.first().copied()? as u64;
            let usecs = bar.data_bar_usecs.first().copied().unwrap_or(0) as u64;
            let time_ms = ssboe * 1000 + (usecs + 500) / 1000;

            let price = bar.close_price?;
            let volume = bar.volume.unwrap_or(0) as f64;

            let side = match (bar.bid_volume, bar.ask_volume) {
                (Some(b), Some(a)) if a > b => Side::Buy,
                (Some(b), Some(a)) if b > a => Side::Sell,
                _ => Side::Sell,
            };

            Some(Trade {
                time: Timestamp::from_millis(time_ms),
                price: Price::from_f64(price),
                quantity: Quantity(volume),
                side,
            })
        }
        RithmicMessage::LastTrade(_) => map_last_trade(&r.message),
        _ => None,
    }));
    trades
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_precision() {
        let price = Price::from_f64(4523.75);
        let back = price.to_f64();
        assert!(
            (back - 4523.75).abs() < 0.01,
            "Price round-trip failed: {} -> {} -> {}",
            4523.75,
            price.units(),
            back
        );
    }
}
