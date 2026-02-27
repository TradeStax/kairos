//! Time-ordered data feed that merges trade and depth streams.
//!
//! [`DataFeed`] accepts pre-loaded trade and depth data for multiple
//! instruments, then yields events one at a time in chronological
//! order via [`DataFeed::next_event`]. This is the primary entry
//! point for the backtest engine's market data replay.

use kairos_data::{Depth, FuturesTicker, Timestamp, Trade};

/// A single data event emitted by the feed during replay.
///
/// Each event carries the originating instrument and either a
/// [`Trade`] or [`Depth`] snapshot. Events are yielded in
/// chronological order by [`DataFeed::next_event`].
#[derive(Debug, Clone)]
pub enum FeedEvent {
    /// An individual trade execution.
    Trade {
        /// The futures instrument this trade belongs to.
        instrument: FuturesTicker,
        /// The trade tick data.
        trade: Trade,
    },
    /// A depth-of-book snapshot.
    Depth {
        /// The futures instrument this snapshot belongs to.
        instrument: FuturesTicker,
        /// The order book depth snapshot.
        depth: Depth,
    },
}

impl FeedEvent {
    /// Returns the timestamp of this event.
    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        match self {
            Self::Trade { trade, .. } => trade.time,
            Self::Depth { depth, .. } => Timestamp(depth.time),
        }
    }

    /// Returns the instrument this event belongs to.
    #[must_use]
    pub fn instrument(&self) -> FuturesTicker {
        match self {
            Self::Trade { instrument, .. } | Self::Depth { instrument, .. } => *instrument,
        }
    }
}

/// A cursor tracking read position within a single instrument's
/// trade data. Advances forward only.
struct TradeCursor {
    instrument: FuturesTicker,
    trades: Vec<Trade>,
    index: usize,
}

/// A cursor tracking read position within a single instrument's
/// depth snapshot data. Advances forward only.
struct DepthCursor {
    instrument: FuturesTicker,
    snapshots: Vec<Depth>,
    index: usize,
}

/// Multi-instrument data feed that merges trade and depth streams
/// into a single time-ordered sequence of [`FeedEvent`]s.
///
/// # Data flow
///
/// 1. Call [`add_trades`](Self::add_trades) and/or
///    [`add_depth`](Self::add_depth) to load pre-sorted data for
///    each instrument.
/// 2. Call [`next_event`](Self::next_event) repeatedly to consume
///    events in chronological order across all instruments and data
///    types. Trades win ties against depth at the same timestamp.
/// 3. When `next_event` returns `None`, all data has been consumed.
///
/// Progress can be tracked via [`total_events`](Self::total_events)
/// and [`events_emitted`](Self::events_emitted).
pub struct DataFeed {
    trade_cursors: Vec<TradeCursor>,
    depth_cursors: Vec<DepthCursor>,
    total_events: usize,
    events_emitted: usize,
}

impl DataFeed {
    /// Creates an empty data feed with no instruments loaded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            trade_cursors: Vec::new(),
            depth_cursors: Vec::new(),
            total_events: 0,
            events_emitted: 0,
        }
    }

    /// Adds a batch of trades for an instrument.
    ///
    /// Trades should be pre-sorted in ascending time order. Multiple
    /// calls for the same instrument create separate cursors that
    /// are merged during replay.
    pub fn add_trades(&mut self, instrument: FuturesTicker, trades: Vec<Trade>) {
        self.total_events += trades.len();
        self.trade_cursors.push(TradeCursor {
            instrument,
            trades,
            index: 0,
        });
    }

    /// Adds a batch of depth snapshots for an instrument.
    ///
    /// Snapshots should be pre-sorted in ascending time order.
    pub fn add_depth(&mut self, instrument: FuturesTicker, snapshots: Vec<Depth>) {
        self.total_events += snapshots.len();
        self.depth_cursors.push(DepthCursor {
            instrument,
            snapshots,
            index: 0,
        });
    }

    /// Total number of events across all loaded instruments.
    #[must_use]
    pub fn total_events(&self) -> usize {
        self.total_events
    }

    /// Number of events consumed so far via [`next_event`](Self::next_event).
    #[must_use]
    pub fn events_emitted(&self) -> usize {
        self.events_emitted
    }

    /// Returns the next event in chronological order across all
    /// cursors, or `None` if all data has been consumed.
    ///
    /// When a trade and a depth snapshot share the same timestamp,
    /// the trade is emitted first.
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
