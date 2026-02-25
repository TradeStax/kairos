use kairos_data::Trade;

/// Synchronous cursor-based iterator over a sorted slice of historical trades.
pub struct HistoricalFeed {
    trades: Vec<Trade>,
    cursor: usize,
}

impl HistoricalFeed {
    pub fn new(trades: Vec<Trade>) -> Self {
        Self { trades, cursor: 0 }
    }

    pub fn total(&self) -> usize {
        self.trades.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

impl Iterator for HistoricalFeed {
    type Item = Trade;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor < self.trades.len() {
            let trade = self.trades[self.cursor];
            self.cursor += 1;
            Some(trade)
        } else {
            None
        }
    }
}
