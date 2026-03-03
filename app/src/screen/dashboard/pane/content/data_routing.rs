use super::Content;

impl Content {
    /// Append a single trade to the active chart/panel.
    pub(crate) fn append_trade(&mut self, trade: &data::Trade) {
        match self {
            Content::Candlestick { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    c.append_trade(trade);
                }
            }
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart: Some(c), .. } => c.append_trade(trade),
            Content::Profile { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    c.append_trade(trade);
                }
            }
            _ => {}
        }
    }

    /// Route a live depth snapshot (with bundled trades) to the content.
    #[allow(dead_code)]
    pub(crate) fn update_live_depth(&mut self, _depth: &data::Depth, _trades: &[data::Trade]) {
        match self {
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart: Some(c), .. } => {
                c.update_from_replay(_depth, _trades);
            }
            #[cfg(feature = "heatmap")]
            Content::Ladder(Some(panel)) => {
                panel.update_from_replay(_depth, _trades);
            }
            _ => {}
        }
    }

    /// Rebuild the chart from scratch with the given trades (used by replay seek).
    pub(crate) fn rebuild_from_trades(&mut self, trades: &[data::Trade]) {
        match self {
            Content::Candlestick { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    c.rebuild_from_trades(trades);
                }
            }
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart: Some(c), .. } => c.rebuild_from_trades(trades),
            Content::Profile { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    c.rebuild_from_trades(trades);
                }
            }
            _ => {}
        }
    }
}
