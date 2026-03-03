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

            // Rithmic TransactionType names the PASSIVE side of the book:
            //   Buy  (1) = bid was resting side → seller aggressed → Sell
            //   Sell (2) = ask was resting side → buyer aggressed → Buy
            let side = match lt.aggressor {
                Some(1) => Side::Sell,
                Some(2) => Side::Buy,
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
    trades.extend(responses.iter().filter_map(|r| match &*r.message {
        RithmicMessage::ResponseTickBarReplay(bar) => {
            let ssboe = bar.data_bar_ssboe.first().copied()? as u64;
            let usecs = bar.data_bar_usecs.first().copied().unwrap_or(0) as u64;
            let time_ms = ssboe * 1000 + (usecs + 500) / 1000;

            let price = bar.close_price?;
            let volume = bar.volume.unwrap_or(0) as f64;

            // Infer aggressor side from volume split. Fallback to Sell
            // when volumes are equal, missing, or both None.
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
    use super::super::protocol::rti::{BestBidOffer, LastTrade, OrderBook, ResponseTickBarReplay};
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

    fn make_last_trade(
        ssboe: Option<i32>,
        usecs: Option<i32>,
        price: Option<f64>,
        size: Option<i32>,
        aggressor: Option<i32>,
    ) -> RithmicMessage {
        RithmicMessage::LastTrade(LastTrade {
            template_id: 150,
            symbol: Some("ESH5".into()),
            exchange: Some("CME".into()),
            presence_bits: None,
            clear_bits: None,
            is_snapshot: None,
            trade_price: price,
            trade_size: size,
            aggressor,
            exchange_order_id: None,
            aggressor_exchange_order_id: None,
            net_change: None,
            percent_change: None,
            volume: None,
            vwap: None,
            trade_time: None,
            ssboe,
            usecs,
            source_ssboe: None,
            source_usecs: None,
            source_nsecs: None,
            jop_ssboe: None,
            jop_nsecs: None,
        })
    }

    #[test]
    fn map_last_trade_buy_side() {
        // aggressor=2 means ask was resting -> buyer aggressed -> Buy
        let msg = make_last_trade(
            Some(1700000000),
            Some(500000),
            Some(5200.25),
            Some(3),
            Some(2),
        );
        let trade = map_last_trade(&msg).unwrap();
        assert_eq!(trade.side, Side::Buy);
        assert!((trade.price.to_f64() - 5200.25).abs() < 0.01);
        assert!((trade.quantity.0 - 3.0).abs() < 0.01);
        // Time: 1700000000 * 1000 + (500000 + 500) / 1000 = 1700000000500
        assert_eq!(trade.time.to_millis(), 1700000000500);
    }

    #[test]
    fn map_last_trade_sell_side() {
        // aggressor=1 means bid was resting -> seller aggressed -> Sell
        let msg = make_last_trade(Some(1700000000), Some(0), Some(5200.0), Some(10), Some(1));
        let trade = map_last_trade(&msg).unwrap();
        assert_eq!(trade.side, Side::Sell);
    }

    #[test]
    fn map_last_trade_unknown_aggressor_defaults_to_sell() {
        let msg = make_last_trade(Some(1700000000), None, Some(5200.0), Some(1), Some(99));
        let trade = map_last_trade(&msg).unwrap();
        assert_eq!(trade.side, Side::Sell);
    }

    #[test]
    fn map_last_trade_none_aggressor_defaults_to_sell() {
        let msg = make_last_trade(Some(1700000000), None, Some(5200.0), Some(1), None);
        let trade = map_last_trade(&msg).unwrap();
        assert_eq!(trade.side, Side::Sell);
    }

    #[test]
    fn map_last_trade_missing_price_returns_none() {
        let msg = make_last_trade(Some(1700000000), None, None, Some(1), Some(2));
        assert!(map_last_trade(&msg).is_none());
    }

    #[test]
    fn map_last_trade_missing_size_returns_none() {
        let msg = make_last_trade(Some(1700000000), None, Some(100.0), None, Some(2));
        assert!(map_last_trade(&msg).is_none());
    }

    #[test]
    fn map_last_trade_missing_ssboe_returns_none() {
        let msg = make_last_trade(None, None, Some(100.0), Some(1), Some(2));
        assert!(map_last_trade(&msg).is_none());
    }

    #[test]
    fn map_last_trade_non_last_trade_variant_returns_none() {
        let msg = RithmicMessage::Unknown;
        assert!(map_last_trade(&msg).is_none());
    }

    #[test]
    fn map_bbo_to_depth_both_sides() {
        let msg = RithmicMessage::BestBidOffer(BestBidOffer {
            template_id: 150,
            symbol: Some("ESH5".into()),
            exchange: Some("CME".into()),
            presence_bits: None,
            clear_bits: None,
            is_snapshot: None,
            bid_price: Some(5200.0),
            bid_size: Some(50),
            bid_orders: None,
            bid_implicit_size: None,
            bid_time: None,
            ask_price: Some(5200.25),
            ask_size: Some(30),
            ask_orders: None,
            ask_implicit_size: None,
            ask_time: None,
            lean_price: None,
            ssboe: Some(1700000000),
            usecs: Some(0),
        });

        let depth = map_bbo_to_depth(&msg).unwrap();
        assert_eq!(depth.bids.len(), 1);
        assert_eq!(depth.asks.len(), 1);
        let (bid_p, bid_q) = depth.best_bid().unwrap();
        assert!((bid_p.to_f64() - 5200.0).abs() < 0.01);
        assert!((bid_q - 50.0).abs() < 0.01);
        let (ask_p, ask_q) = depth.best_ask().unwrap();
        assert!((ask_p.to_f64() - 5200.25).abs() < 0.01);
        assert!((ask_q - 30.0).abs() < 0.01);
    }

    #[test]
    fn map_orderbook_to_depth_basic() {
        let msg = RithmicMessage::OrderBook(OrderBook {
            template_id: 150,
            symbol: Some("ESH5".into()),
            exchange: Some("CME".into()),
            presence_bits: None,
            update_type: None,
            bid_price: vec![5200.0, 5199.75],
            bid_size: vec![100, 50],
            bid_orders: vec![],
            impl_bid_size: vec![],
            ask_price: vec![5200.25, 5200.50],
            ask_size: vec![80, 40],
            ask_orders: vec![],
            impl_ask_size: vec![],
            ssboe: Some(1700000000),
            usecs: Some(0),
        });

        let depth = map_orderbook_to_depth(&msg).unwrap();
        assert_eq!(depth.bids.len(), 2);
        assert_eq!(depth.asks.len(), 2);
    }

    #[test]
    fn map_orderbook_mismatched_sizes_returns_none() {
        let msg = RithmicMessage::OrderBook(OrderBook {
            template_id: 150,
            symbol: None,
            exchange: None,
            presence_bits: None,
            update_type: None,
            bid_price: vec![5200.0, 5199.75],
            bid_size: vec![100], // mismatch: 2 prices, 1 size
            bid_orders: vec![],
            impl_bid_size: vec![],
            ask_price: vec![],
            ask_size: vec![],
            ask_orders: vec![],
            impl_ask_size: vec![],
            ssboe: Some(1700000000),
            usecs: Some(0),
        });
        assert!(map_orderbook_to_depth(&msg).is_none());
    }

    #[test]
    fn map_orderbook_empty_book_returns_none() {
        let msg = RithmicMessage::OrderBook(OrderBook {
            template_id: 150,
            symbol: None,
            exchange: None,
            presence_bits: None,
            update_type: None,
            bid_price: vec![],
            bid_size: vec![],
            bid_orders: vec![],
            impl_bid_size: vec![],
            ask_price: vec![],
            ask_size: vec![],
            ask_orders: vec![],
            impl_ask_size: vec![],
            ssboe: Some(1700000000),
            usecs: Some(0),
        });
        assert!(map_orderbook_to_depth(&msg).is_none());
    }

    #[test]
    fn map_tick_replay_buy_side_inference() {
        let resp = RithmicResponse {
            request_id: String::new(),
            message: Box::new(RithmicMessage::ResponseTickBarReplay(
                ResponseTickBarReplay {
                    template_id: 0,
                    request_key: None,
                    user_msg: vec![],
                    rq_handler_rp_code: vec![],
                    rp_code: vec![],
                    symbol: Some("ES".into()),
                    exchange: Some("CME".into()),
                    r#type: None,
                    sub_type: None,
                    type_specifier: None,
                    num_trades: None,
                    volume: Some(5),
                    bid_volume: Some(1),
                    ask_volume: Some(4), // ask > bid -> Buy
                    open_price: None,
                    close_price: Some(5200.0),
                    high_price: None,
                    low_price: None,
                    custom_session_open_ssm: None,
                    data_bar_ssboe: vec![1700000000],
                    data_bar_usecs: vec![0],
                },
            )),
            is_update: false,
            has_more: false,
            multi_response: false,
            error: None,
            source: String::new(),
        };

        let trades = map_tick_replay_to_trades(&[resp]);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].side, Side::Buy);
        assert!((trades[0].price.to_f64() - 5200.0).abs() < 0.01);
        assert!((trades[0].quantity.0 - 5.0).abs() < 0.01);
    }

    #[test]
    fn map_tick_replay_sell_side_inference() {
        let resp = RithmicResponse {
            request_id: String::new(),
            message: Box::new(RithmicMessage::ResponseTickBarReplay(
                ResponseTickBarReplay {
                    template_id: 0,
                    request_key: None,
                    user_msg: vec![],
                    rq_handler_rp_code: vec![],
                    rp_code: vec![],
                    symbol: None,
                    exchange: None,
                    r#type: None,
                    sub_type: None,
                    type_specifier: None,
                    num_trades: None,
                    volume: Some(10),
                    bid_volume: Some(7), // bid > ask -> Sell
                    ask_volume: Some(3),
                    open_price: None,
                    close_price: Some(5199.75),
                    high_price: None,
                    low_price: None,
                    custom_session_open_ssm: None,
                    data_bar_ssboe: vec![1700000000],
                    data_bar_usecs: vec![],
                },
            )),
            is_update: false,
            has_more: false,
            multi_response: false,
            error: None,
            source: String::new(),
        };

        let trades = map_tick_replay_to_trades(&[resp]);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].side, Side::Sell);
    }

    #[test]
    fn map_tick_replay_zero_volume() {
        let resp = RithmicResponse {
            request_id: String::new(),
            message: Box::new(RithmicMessage::ResponseTickBarReplay(
                ResponseTickBarReplay {
                    template_id: 0,
                    request_key: None,
                    user_msg: vec![],
                    rq_handler_rp_code: vec![],
                    rp_code: vec![],
                    symbol: None,
                    exchange: None,
                    r#type: None,
                    sub_type: None,
                    type_specifier: None,
                    num_trades: None,
                    volume: None, // None volume -> defaults to 0
                    bid_volume: None,
                    ask_volume: None,
                    open_price: None,
                    close_price: Some(100.0),
                    high_price: None,
                    low_price: None,
                    custom_session_open_ssm: None,
                    data_bar_ssboe: vec![1000],
                    data_bar_usecs: vec![],
                },
            )),
            is_update: false,
            has_more: false,
            multi_response: false,
            error: None,
            source: String::new(),
        };

        let trades = map_tick_replay_to_trades(&[resp]);
        assert_eq!(trades.len(), 1);
        assert!((trades[0].quantity.0).abs() < 0.01);
    }

    #[test]
    fn map_tick_replay_missing_close_price_skipped() {
        let resp = RithmicResponse {
            request_id: String::new(),
            message: Box::new(RithmicMessage::ResponseTickBarReplay(
                ResponseTickBarReplay {
                    template_id: 0,
                    request_key: None,
                    user_msg: vec![],
                    rq_handler_rp_code: vec![],
                    rp_code: vec![],
                    symbol: None,
                    exchange: None,
                    r#type: None,
                    sub_type: None,
                    type_specifier: None,
                    num_trades: None,
                    volume: Some(10),
                    bid_volume: None,
                    ask_volume: None,
                    open_price: None,
                    close_price: None, // missing -> skipped
                    high_price: None,
                    low_price: None,
                    custom_session_open_ssm: None,
                    data_bar_ssboe: vec![1000],
                    data_bar_usecs: vec![],
                },
            )),
            is_update: false,
            has_more: false,
            multi_response: false,
            error: None,
            source: String::new(),
        };

        let trades = map_tick_replay_to_trades(&[resp]);
        assert!(trades.is_empty());
    }
}
