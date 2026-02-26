use crate::engine::kernel::Engine;
use crate::fill::FillResult;
use crate::output::progress::BacktestProgressEvent;
use crate::output::trade_record::ExitReason;
use crate::strategy::{OrderEvent, Strategy};
use kairos_data::Trade;
use uuid::Uuid;

impl Engine {
    pub(crate) fn check_fills(
        &mut self,
        trade: &Trade,
        run_id: Uuid,
        sender: Option<&'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>>,
        strategy: &mut dyn Strategy,
    ) {
        let active: Vec<_> = self.order_book.active_orders().collect();
        if active.is_empty() {
            return;
        }

        let primary_depth = self.latest_depth.get(&self.config.ticker);

        let fills =
            self.fill_simulator
                .check_fills(trade, primary_depth, &active, &self.instruments);

        for fill in fills {
            self.process_fill(fill, trade, run_id, sender, strategy);
        }
    }

    pub(crate) fn process_fill(
        &mut self,
        fill: FillResult,
        trade: &Trade,
        run_id: Uuid,
        sender: Option<&'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>>,
        strategy: &mut dyn Strategy,
    ) {
        let order_id = fill.order_id;

        // Record fill on the order
        if let Some(order) = self.order_book.get_mut(order_id) {
            let instrument = order.instrument;
            let side = order.side;
            let label = order.label.clone();
            let is_filled_now = {
                order.record_fill(fill.fill_quantity, fill.fill_price, fill.timestamp);
                order.status == crate::order::types::OrderStatus::Filled
            };

            // If this was a bracket entry, activate children
            if is_filled_now {
                self.order_book.activate_bracket_children(order_id);
            }

            // Cancel OCO partner if filled
            if is_filled_now
                && let Some(order) = self.order_book.get(order_id)
                && let Some(oco) = order.oco_partner
            {
                self.order_book.cancel(oco, fill.timestamp);
            }

            // Determine exit reason from order context
            let exit_reason = if let Some(order) = self.order_book.get(order_id) {
                match &order.label {
                    Some(l) if l.contains("SL") => Some(ExitReason::BracketSL),
                    Some(l) if l.contains("TP") => Some(ExitReason::BracketTP),
                    _ => None,
                }
            } else {
                None
            };

            // Update portfolio
            let trade_record = self.portfolio.process_fill(
                instrument,
                side,
                fill.fill_price,
                fill.fill_quantity,
                fill.timestamp,
                exit_reason,
                label,
            );

            // If this fill opened a new position from a bracket
            // entry, set the stop loss on it.
            if trade_record.is_none()
                && let Some(sl_price) = self.order_book.bracket_stop_loss(order_id)
                && let Some(pos) = self.portfolio.positions_mut().get_mut(&instrument)
            {
                pos.set_stop_loss(sl_price);
            }

            if let Some(record) = trade_record {
                if let Some(s) = sender {
                    let _ = s.send(BacktestProgressEvent::TradeCompleted {
                        run_id,
                        trade: Box::new(record.clone()),
                    });
                }
                self.equity_curve
                    .record(fill.timestamp, self.portfolio.cash(), 0.0);
                self.completed_trades.push(record);
            }

            // Notify strategy of fill
            let primary = self.config.ticker;
            self.rebuild_context_cache(primary);
            let requests = {
                let ctx = self.build_context(primary, trade);
                let event = OrderEvent::Filled {
                    order_id,
                    fill_price: fill.fill_price,
                    fill_quantity: fill.fill_quantity,
                };
                strategy.on_order_event(event, &ctx)
            };
            self.process_order_requests(requests, trade, run_id, sender);
        }
    }
}
