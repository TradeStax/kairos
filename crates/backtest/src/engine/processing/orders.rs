use crate::engine::kernel::Engine;
use crate::order::request::{BracketOrder, NewOrder, OrderRequest};
use crate::order::types::OrderType;
use crate::output::progress::BacktestProgressEvent;
use crate::output::trade_record::ExitReason;
use kairos_data::{FuturesTicker, Trade};
use uuid::Uuid;

impl Engine {
    pub(crate) fn process_order_requests(
        &mut self,
        requests: Vec<OrderRequest>,
        trade: &Trade,
        _run_id: Uuid,
        _sender: Option<&'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>>,
    ) {
        for request in requests {
            match request {
                OrderRequest::Submit(new_order) => {
                    self.submit_order(new_order, trade);
                }
                OrderRequest::SubmitBracket(bracket) => {
                    self.submit_bracket(bracket, trade);
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
                    self.order_book
                        .modify(order_id, new_price, new_quantity, trade.time);
                }
                OrderRequest::Flatten { instrument, reason } => {
                    self.flatten_position(instrument, reason, trade);
                }
                OrderRequest::Noop => {}
            }
        }
    }

    pub(crate) fn submit_order(&mut self, new_order: NewOrder, trade: &Trade) {
        // Margin check
        if !new_order.reduce_only
            && !self
                .portfolio
                .check_margin(&new_order.instrument, new_order.quantity)
        {
            return;
        }

        let id = self.order_book.create_order(&new_order, trade.time);

        // Market orders fill immediately
        if matches!(new_order.order_type, OrderType::Market) {
            let instrument = self.instruments.get(&new_order.instrument);
            if let Some(inst) = instrument {
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

                // Update portfolio
                let record = self.portfolio.process_fill(
                    new_order.instrument,
                    new_order.side,
                    fill_price,
                    new_order.quantity,
                    trade.time,
                    None,
                    new_order.label,
                );

                if let Some(record) = record {
                    self.equity_curve
                        .record(trade.time, self.portfolio.cash(), 0.0);
                    self.completed_trades.push(record);
                }
            }
        }
    }

    pub(crate) fn submit_bracket(&mut self, bracket: BracketOrder, trade: &Trade) {
        // Margin check
        if !self
            .portfolio
            .check_margin(&bracket.entry.instrument, bracket.entry.quantity)
        {
            return;
        }

        let (entry_id, _sl_id, _tp_id) = self.order_book.create_bracket(&bracket, trade.time);

        // If entry is Market, fill immediately
        if matches!(bracket.entry.order_type, OrderType::Market) {
            let instrument = self.instruments.get(&bracket.entry.instrument);
            if let Some(inst) = instrument {
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
                // Activate bracket children
                self.order_book.activate_bracket_children(entry_id);

                // Update portfolio
                let record = self.portfolio.process_fill(
                    bracket.entry.instrument,
                    bracket.entry.side,
                    fill_price,
                    bracket.entry.quantity,
                    trade.time,
                    None,
                    bracket.entry.label,
                );

                // Set stop loss on the newly created position
                if let Some(pos) = self
                    .portfolio
                    .positions_mut()
                    .get_mut(&bracket.entry.instrument)
                {
                    pos.set_stop_loss(bracket.stop_loss);
                }

                if let Some(record) = record {
                    self.equity_curve
                        .record(trade.time, self.portfolio.cash(), 0.0);
                    self.completed_trades.push(record);
                }
            }
        }
    }

    pub(crate) fn flatten_position(
        &mut self,
        instrument: FuturesTicker,
        reason: ExitReason,
        trade: &Trade,
    ) {
        // Cancel all orders for this instrument
        self.order_book.cancel_all(Some(instrument), trade.time);

        // Close position at market
        if let Some(pos) = self.portfolio.positions().get(&instrument) {
            let side = pos.side.opposite();
            let qty = pos.quantity;
            let inst = self.instruments.get(&instrument);
            if let Some(inst) = inst {
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
                if let Some(record) = record {
                    self.equity_curve
                        .record(trade.time, self.portfolio.cash(), 0.0);
                    self.completed_trades.push(record);
                }
            }
        }
    }

    pub(crate) fn close_all_positions(&mut self, reason: ExitReason) {
        let instruments: Vec<FuturesTicker> = self.portfolio.positions().keys().copied().collect();

        for &instrument in &instruments {
            if let Some(pos) = self.portfolio.positions().get(&instrument) {
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
