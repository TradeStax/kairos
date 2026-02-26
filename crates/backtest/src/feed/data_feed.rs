use kairos_data::{Depth, FuturesTicker, Timestamp, Trade};

/// A single data event from the feed.
#[derive(Debug, Clone)]
pub enum FeedEvent {
    Trade {
        instrument: FuturesTicker,
        trade: Trade,
    },
    Depth {
        instrument: FuturesTicker,
        depth: Depth,
    },
}

impl FeedEvent {
    pub fn timestamp(&self) -> Timestamp {
        match self {
            Self::Trade { trade, .. } => trade.time,
            Self::Depth { depth, .. } => Timestamp(depth.time),
        }
    }

    pub fn instrument(&self) -> FuturesTicker {
        match self {
            Self::Trade { instrument, .. } => *instrument,
            Self::Depth { instrument, .. } => *instrument,
        }
    }
}

/// A cursor into a single instrument's trade data.
struct TradeCursor {
    instrument: FuturesTicker,
    trades: Vec<Trade>,
    index: usize,
}

/// A cursor into a single instrument's depth data.
struct DepthCursor {
    instrument: FuturesTicker,
    snapshots: Vec<Depth>,
    index: usize,
}

/// Multi-instrument data feed that merges all data streams
/// into a single time-ordered sequence of events.
pub struct DataFeed {
    trade_cursors: Vec<TradeCursor>,
    depth_cursors: Vec<DepthCursor>,
    total_events: usize,
    events_emitted: usize,
}

impl DataFeed {
    pub fn new() -> Self {
        Self {
            trade_cursors: Vec::new(),
            depth_cursors: Vec::new(),
            total_events: 0,
            events_emitted: 0,
        }
    }

    /// Add trades for an instrument.
    pub fn add_trades(&mut self, instrument: FuturesTicker, trades: Vec<Trade>) {
        self.total_events += trades.len();
        self.trade_cursors.push(TradeCursor {
            instrument,
            trades,
            index: 0,
        });
    }

    /// Add depth snapshots for an instrument.
    pub fn add_depth(&mut self, instrument: FuturesTicker, snapshots: Vec<Depth>) {
        self.total_events += snapshots.len();
        self.depth_cursors.push(DepthCursor {
            instrument,
            snapshots,
            index: 0,
        });
    }

    pub fn total_events(&self) -> usize {
        self.total_events
    }

    pub fn events_emitted(&self) -> usize {
        self.events_emitted
    }

    /// Get the next event in time order across all cursors.
    pub fn next_event(&mut self) -> Option<FeedEvent> {
        // Find the earliest trade across all trade cursors
        let earliest_trade = self
            .trade_cursors
            .iter()
            .enumerate()
            .filter(|(_, c)| c.index < c.trades.len())
            .min_by_key(|(_, c)| c.trades[c.index].time.0)
            .map(|(i, c)| (i, c.trades[c.index].time.0, true));

        // Find the earliest depth across all depth cursors
        let earliest_depth = self
            .depth_cursors
            .iter()
            .enumerate()
            .filter(|(_, c)| c.index < c.snapshots.len())
            .min_by_key(|(_, c)| c.snapshots[c.index].time)
            .map(|(i, c)| (i, c.snapshots[c.index].time, false));

        // Pick whichever is earlier (trades win ties)
        let (cursor_idx, _ts, is_trade) = match (earliest_trade, earliest_depth) {
            (Some(t), Some(d)) => {
                if t.1 <= d.1 {
                    t
                } else {
                    d
                }
            }
            (Some(t), None) => t,
            (None, Some(d)) => d,
            (None, None) => return None,
        };

        self.events_emitted += 1;

        if is_trade {
            let cursor = &mut self.trade_cursors[cursor_idx];
            let trade = cursor.trades[cursor.index];
            let instrument = cursor.instrument;
            cursor.index += 1;
            Some(FeedEvent::Trade { instrument, trade })
        } else {
            let cursor = &mut self.depth_cursors[cursor_idx];
            let depth = cursor.snapshots[cursor.index].clone();
            let instrument = cursor.instrument;
            cursor.index += 1;
            Some(FeedEvent::Depth { instrument, depth })
        }
    }
}

impl Default for DataFeed {
    fn default() -> Self {
        Self::new()
    }
}
