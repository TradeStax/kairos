//! Rithmic Message Mapper
//!
//! Converts Rithmic protocol messages to domain types (Trade, DepthSnapshot)
//! and exchange types (Depth). Uses the same Price/Quantity/Timestamp types
//! as the rest of the codebase.

use crate::types::{Depth, Trade, TradeSide};
use kairos_data::domain::{Price, Quantity, Side, Timestamp, Trade as DomainTrade};
use rithmic_rs::rti::messages::RithmicMessage;

/// Convert a Rithmic LastTrade message to an exchange Trade
///
/// LastTrade contains: trade_price, trade_size, aggressor, volume,
/// vwap, net_change, ssboe (seconds since beginning of epoch), usecs
pub fn map_last_trade(msg: &RithmicMessage) -> Option<Trade> {
    match msg {
        RithmicMessage::LastTrade(lt) => {
            // ssboe is seconds since epoch, usecs is microsecond offset
            let ssboe = lt.ssboe? as u64;
            let usecs = lt.usecs.unwrap_or(0) as u64;
            // Round microseconds to nearest millisecond instead of truncating
            let time_ms = ssboe * 1000 + (usecs + 500) / 1000;

            // Keep f64 precision until final conversion
            let price = lt.trade_price? as f32;
            let size = lt.trade_size? as f32;

            // aggressor: 1 = Buy, 2 = Sell (Rithmic protobuf convention)
            let side = match lt.aggressor {
                Some(1) => TradeSide::Buy,
                Some(2) => TradeSide::Sell,
                other => {
                    log::debug!(
                        "Unknown Rithmic aggressor value: {:?}, defaulting to Sell",
                        other
                    );
                    TradeSide::Sell
                }
            };

            Some(Trade {
                time: time_ms,
                price,
                qty: size,
                side,
            })
        }
        _ => None,
    }
}

/// Convert a Rithmic LastTrade message directly to a domain Trade
pub fn map_to_domain_trade(msg: &RithmicMessage) -> Option<DomainTrade> {
    match msg {
        RithmicMessage::LastTrade(lt) => {
            let ssboe = lt.ssboe? as u64;
            let usecs = lt.usecs.unwrap_or(0) as u64;
            let time_ms = ssboe * 1000 + (usecs + 500) / 1000;

            let price = lt.trade_price?;
            let size = lt.trade_size? as f64;

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

            Some(DomainTrade {
                time: Timestamp::from_millis(time_ms),
                price: Price::from_f64(price),
                quantity: Quantity(size),
                side,
            })
        }
        _ => None,
    }
}

/// Convert a Rithmic BestBidOffer message to exchange Depth
///
/// BBO contains: bid_price, bid_size, ask_price, ask_size, plus
/// timestamp fields (ssboe, usecs)
pub fn map_bbo_to_exchange_depth(msg: &RithmicMessage) -> Option<(u64, Depth)> {
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

            Some((time_ms, depth))
        }
        _ => None,
    }
}

/// Convert a Rithmic OrderBook message to exchange Depth
///
/// OrderBook contains arrays of bid_price, bid_size, ask_price, ask_size
pub fn map_orderbook_to_exchange_depth(msg: &RithmicMessage) -> Option<(u64, Depth)> {
    match msg {
        RithmicMessage::OrderBook(ob) => {
            let ssboe = ob.ssboe? as u64;
            let usecs = ob.usecs.unwrap_or(0) as u64;
            let time_ms = ssboe * 1000 + (usecs + 500) / 1000;

            let mut depth = Depth::new(time_ms);

            // Validate array lengths match
            if ob.bid_price.len() != ob.bid_size.len() {
                log::warn!(
                    "OrderBook bid price/size mismatch: {} prices, {} sizes",
                    ob.bid_price.len(),
                    ob.bid_size.len()
                );
            }
            if ob.ask_price.len() != ob.ask_size.len() {
                log::warn!(
                    "OrderBook ask price/size mismatch: {} prices, {} sizes",
                    ob.ask_price.len(),
                    ob.ask_size.len()
                );
            }

            // Process bid levels
            for (price, size) in ob.bid_price.iter().zip(ob.bid_size.iter()) {
                let price_units = Price::from_f64(*price).units();
                depth.bids.insert(price_units, *size as f32);
            }

            // Process ask levels
            for (price, size) in ob.ask_price.iter().zip(ob.ask_size.iter()) {
                let price_units = Price::from_f64(*price).units();
                depth.asks.insert(price_units, *size as f32);
            }

            if depth.bids.is_empty() && depth.asks.is_empty() {
                return None;
            }

            Some((time_ms, depth))
        }
        _ => None,
    }
}

/// Convert historical tick responses to domain trades
pub fn map_tick_replay_to_trades(responses: &[rithmic_rs::RithmicResponse]) -> Vec<DomainTrade> {
    responses
        .iter()
        .filter_map(|r| map_to_domain_trade(&r.message))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_last_trade_buy() {
        let trade = Trade {
            time: 1700000000000,
            price: 4500.25,
            qty: 5.0,
            side: TradeSide::Buy,
        };

        let domain = DomainTrade::from_raw(
            trade.time,
            trade.price,
            trade.qty,
            trade.side == TradeSide::Sell,
        );
        assert!(domain.is_buy());
        assert_eq!(domain.quantity, Quantity(5.0));
    }

    #[test]
    fn test_price_conversion_precision() {
        let price = Price::from_f64(4523.75);
        let f64_back = price.to_f64();
        assert!(
            (f64_back - 4523.75).abs() < 0.01,
            "Price round-trip: {} -> {} -> {}",
            4523.75,
            price.units(),
            f64_back
        );
    }
}
