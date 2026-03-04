//! Order submission, modification, and position flattening.
//!
//! Handles all [`OrderRequest`] variants dispatched by the
//! strategy: single orders, bracket orders, cancellations,
//! modifications, and position flattening.

use crate::engine::kernel::Engine;
use crate::order::request::{BracketOrder, NewOrder, OrderRequest};
use crate::order::types::OrderType;
use crate::output::progress::BacktestProgressEvent;
use crate::output::trade_record::ExitReason;
use crate::strategy::Strategy;
use kairos_data::{FuturesTicker, Trade};
use uuid::Uuid;

impl Engine {
    /// Dispatches a batch of order requests from a strategy
    /// callback.
    ///
    /// Each request is processed sequentially to maintain
    /// deterministic ordering — a cancel must take effect before
    /// a subsequent submit in the same batch.
    pub(crate) fn process_order_requests(
        &mut self,
        requests: Vec<OrderRequest>,
        trade: &Trade,
        _run_id: Uuid,
        _sender: Option<&'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>>,
        strategy: &dyn Strategy,
    ) {
        for request in requests {
            match request {
                OrderRequest::Submit(new_order) => {
                    self.submit_order(new_order, trade, strategy);
                }
                OrderRequest::SubmitBracket(bracket) => {
                    self.submit_bracket(bracket, trade, strategy);
                }
                OrderRequest::Cancel { order_id } => {
                    self.order_book.cancel(order_id, trade.time);
                }
                OrderRequest::CancelAll { instrument } => {
                    self.order_book.cancel_all(instrument, trade.time);
                }
                OrderRequest::Modify {
                    order_id,
                    new_price,
                    new_quantity,
                } => {
                    let _ = self
                        .order_book
                        .modify(order_id, new_price, new_quantity, trade.time);
                }
                OrderRequest::Flatten { instrument, reason } => {
                    self.flatten_position(instrument, reason, trade, strategy);
                }
                OrderRequest::Noop => {}
            }
        }
    }

    /// Submits a single order to the order book.
    ///
    /// Market orders are filled immediately using the fill
    /// simulator. Limit/stop orders are added to the book and
    /// checked against future trades. A margin check is performed
    /// before submission (unless `reduce_only` is set).
    pub(crate) fn submit_order(
        &mut self,
        new_order: NewOrder,
        trade: &Trade,
        strategy: &dyn Strategy,
    ) {
        // Margin check for non-reduce-only orders
        if !new_order.reduce_only
            && !self
                .portfolio
                .check_margin(&new_order.instrument, new_order.quantity)
        {
            return;
        }

        let id = self.order_book.create_order(&new_order, trade.time);

        // Market orders fill immediately at simulated price
        if matches!(new_order.order_type, OrderType::Market) {
            let Some(inst) = self.instruments.get(&new_order.instrument) else {
                return;
            };

            let fill_price = self.fill_simulator.market_fill_price(
                trade,
                new_order.side,
                new_order.quantity,
                self.latest_depth.get(&new_order.instrument),
                inst,
            );
            if let Some(order) = self.order_book.get_mut(id) {
                order.record_fill(new_order.quantity, fill_price, trade.time);
            }

            let record = self.portfolio.process_fill(
                new_order.instrument,
                new_order.side,
                fill_price,
                new_order.quantity,
                trade.time,
                None,
                new_order.label,
            );

            if let Some(mut record) = record {
                record.snapshot = Some(self.build_trade_snapshot(&record, trade, strategy));
                self.equity_curve
                    .record(trade.time, self.portfolio.cash(), 0.0);
                self.completed_trades.push(record);
            }
        }
    }

    /// Submits a bracket order (entry + stop-loss + take-profit).
    ///
    /// The entry order is created with pending SL/TP children. If
    /// the entry is a market order, it fills immediately, the
    /// bracket children are activated, and the stop loss is set on
    /// the resulting position.
    pub(crate) fn submit_bracket(
        &mut self,
        bracket: BracketOrder,
        trade: &Trade,
        strategy: &dyn Strategy,
    ) {
        if !self
            .portfolio
            .check_margin(&bracket.entry.instrument, bracket.entry.quantity)
        {
            return;
        }

        let (entry_id, _sl_id, _tp_id) = self.order_book.create_bracket(&bracket, trade.time);

        // Market entry fills immediately
        if matches!(bracket.entry.order_type, OrderType::Market) {
            let Some(inst) = self.instruments.get(&bracket.entry.instrument) else {
                return;
            };

            let fill_price = self.fill_simulator.market_fill_price(
                trade,
                bracket.entry.side,
                bracket.entry.quantity,
                self.latest_depth.get(&bracket.entry.instrument),
                inst,
            );
            if let Some(order) = self.order_book.get_mut(entry_id) {
                order.record_fill(bracket.entry.quantity, fill_price, trade.time);
            }
            // Activate SL/TP children now that entry is filled
            self.order_book.activate_bracket_children(entry_id);

            let record = self.portfolio.process_fill(
                bracket.entry.instrument,
                bracket.entry.side,
                fill_price,
                bracket.entry.quantity,
                trade.time,
                None,
                bracket.entry.label,
            );

            // Propagate stop loss to the new position
            if let Some(pos) = self
                .portfolio
                .positions_mut()
                .get_mut(&bracket.entry.instrument)
            {
                pos.set_stop_loss(bracket.stop_loss);
            }

            if let Some(mut record) = record {
                record.snapshot = Some(self.build_trade_snapshot(&record, trade, strategy));
                self.equity_curve
                    .record(trade.time, self.portfolio.cash(), 0.0);
                self.completed_trades.push(record);
            }
        }
    }

    /// Flattens (closes) an entire position for the given
    /// instrument.
    ///
    /// Cancels all active orders for the instrument, then submits
    /// a market order in the opposite direction to close the
    /// position.
    pub(crate) fn flatten_position(
        &mut self,
        instrument: FuturesTicker,
        reason: ExitReason,
        trade: &Trade,
        strategy: &dyn Strategy,
    ) {
        self.order_book.cancel_all(Some(instrument), trade.time);

        let Some(pos) = self.portfolio.positions().get(&instrument) else {
            return;
        };
        let side = pos.side.opposite();
        let qty = pos.quantity;

        let Some(inst) = self.instruments.get(&instrument) else {
            return;
        };

        let fill_price = self.fill_simulator.market_fill_price(
            trade,
            side,
            qty,
            self.latest_depth.get(&instrument),
            inst,
        );
        let record = self.portfolio.process_fill(
            instrument,
            side,
            fill_price,
            qty,
            trade.time,
            Some(reason),
            None,
        );
        if let Some(mut record) = record {
            record.snapshot = Some(self.build_trade_snapshot(&record, trade, strategy));
            self.equity_curve
                .record(trade.time, self.portfolio.cash(), 0.0);
            self.completed_trades.push(record);
        }
    }

    /// Closes all open positions at the last available price.
    ///
    /// Used at end-of-data to ensure no positions remain open.
    /// Adds a warning to the result if any positions were
    /// force-closed. Snapshots are attached with candle data but
    /// empty strategy context (forced exit, not strategy-driven).
    pub(crate) fn close_all_positions(&mut self, reason: ExitReason) {
        let instruments: Vec<FuturesTicker> = self.portfolio.positions().keys().copied().collect();

        for &instrument in &instruments {
            let Some(pos) = self.portfolio.positions().get(&instrument) else {
                continue;
            };

            let side = pos.side.opposite();
            let qty = pos.quantity;
            let mark = pos.mark_price;
            let label = pos.label.clone();
            let timestamp = self.clock.now();

            let fill_price = self.latest_prices.get(&instrument).copied().unwrap_or(mark);

            let record = self.portfolio.process_fill(
                instrument,
                side,
                fill_price,
                qty,
                timestamp,
                Some(reason),
                label,
            );
            if let Some(record) = record {
                self.equity_curve
                    .record(timestamp, self.portfolio.cash(), 0.0);
                self.completed_trades.push(record);
            }
        }

        if !instruments.is_empty() {
            self.warnings.push(
                "Position(s) still open at end of data \
                 — closed at last available price."
                    .to_string(),
            );
        }
    }
}
